//! Bounded display-only output from the two Bun pipes. Never wakes collect.
use super::{AppState, Lifecycle, Run, RunError, StatusRequest, manager_run, scripts::now};
use serde::Serialize;
use std::{collections::{BTreeMap, VecDeque}, sync::{Arc, Mutex, Weak}};

const LINE_COUNT: usize = 5;
const LINE_BYTES: usize = 512;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status { Running, Succeeded, Failed }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScriptOutputSnapshot {
    run_id: String,
    observed_at: u64,
    nodes: Vec<NodeOutput>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeOutput {
    node_id: String,
    node_name: String,
    execution_id: String,
    status: Status,
    lines: Vec<String>,
    last_output_at: Option<u64>,
}

struct Entry { execution_id: String, tail: Arc<Mutex<Tail>> }

#[derive(Default)]
pub(super) struct Store(Mutex<BTreeMap<String, BTreeMap<String, Entry>>>);

impl Store {
    pub(super) fn start(&self, run: &str, node: &str, execution: &str) -> Weak<Mutex<Tail>> {
        let tail = Arc::new(Mutex::new(Tail::default()));
        // Superseded processes may still be running. Only the current entry
        // owns the tail, so they stop recording without affecting execution.
        let writer = Arc::downgrade(&tail);
        self.0.lock().unwrap().entry(run.to_owned()).or_default().insert(node.to_owned(), Entry {
            execution_id: execution.to_owned(), tail,
        });
        writer
    }

    pub(super) fn retain_current(&self, run: &Run) {
        let mut runs = self.0.lock().unwrap();
        if run.status == "completed" {
            runs.remove(&run.id);
        } else if let Some(nodes) = runs.get_mut(&run.id) {
            nodes.retain(|node_id, entry| run.nodes.get(node_id).and_then(|node| node.execution.as_ref())
                .is_some_and(|execution| execution.id == entry.execution_id));
            if nodes.is_empty() { runs.remove(&run.id); }
        }
    }

    fn snapshot(&self, run: Run) -> ScriptOutputSnapshot {
        let entries = self.0.lock().unwrap();
        let mut nodes = Vec::new();
        if let Some(tails) = entries.get(&run.id).filter(|_| run.status != "completed") {
            nodes.reserve(tails.len());
            for (node_id, entry) in tails {
                let Some(execution) = run.nodes.get(node_id).and_then(|node| node.execution.as_ref()) else { continue; };
                let Some(script) = execution.script().filter(|_| execution.id == entry.execution_id) else { continue; };
                let tail = entry.tail.lock().unwrap();
                let status = match script.lifecycle {
                    Lifecycle::Running { .. } => tail.ended.unwrap_or(Status::Running),
                    Lifecycle::Succeeded { .. } => Status::Succeeded,
                    Lifecycle::Failed { .. } => Status::Failed,
                };
                nodes.push(NodeOutput {
                    node_id: node_id.clone(),
                    node_name: run.naming.as_ref().and_then(|naming| naming.node_titles.get(node_id)).unwrap_or(node_id).clone(),
                    execution_id: execution.id.clone(), status, lines: tail.lines(), last_output_at: tail.last_output_at,
                });
            }
        }
        ScriptOutputSnapshot { run_id: run.id, observed_at: now(), nodes }
    }
}

pub(crate) async fn manager_script_output(state: AppState, input: StatusRequest) -> Result<ScriptOutputSnapshot, RunError> {
    let run = manager_run(&state, &input.session_id).await?;
    Ok(state.runs.script_output.snapshot(run))
}

#[derive(Clone, Copy)]
pub(super) enum Pipe { Stdout, Stderr }

#[derive(Default)]
pub(super) struct Tail {
    pipes: [LineReader; 2],
    complete: VecDeque<Line>,
    sequence: u64,
    last_output_at: Option<u64>,
    ended: Option<Status>,
}

impl Tail {
    // Lock acquisition is the observation order. Separate pipes cannot recover
    // the child's write order, and never share a partial line or UTF-8 decoder.
    pub(super) fn append(&mut self, pipe: Pipe, bytes: &[u8]) {
        if bytes.is_empty() { return; }
        self.last_output_at = Some(now());
        let reader = &mut self.pipes[pipe as usize];
        for part in bytes.split_inclusive(|byte| *byte == b'\n') {
            if let Some(line) = part.strip_suffix(b"\n") {
                reader.decode(line);
                reader.flush_utf8();
                reader.escape = Escape::Text;
                self.sequence += 1;
                reader.pending.sequence = self.sequence;
                if self.complete.len() == LINE_COUNT { self.complete.pop_front(); }
                self.complete.push_back(std::mem::take(&mut reader.pending));
            } else if reader.decode(part) {
                self.sequence += 1;
                reader.pending.sequence = self.sequence;
            }
        }
    }

    pub(super) fn end_pipe(&mut self, pipe: Pipe) {
        let reader = &mut self.pipes[pipe as usize];
        if reader.flush_utf8() {
            self.sequence += 1;
            reader.pending.sequence = self.sequence;
        }
    }

    pub(super) fn end(&mut self, succeeded: bool) {
        self.ended = Some(if succeeded { Status::Succeeded } else { Status::Failed });
    }

    fn lines(&self) -> Vec<String> {
        let mut recent = [None; LINE_COUNT + 2];
        let lines = self.complete.iter().chain(self.pipes.iter().map(|reader| &reader.pending).filter(|line| line.sequence != 0));
        let mut count = 0;
        for (slot, line) in recent.iter_mut().zip(lines) { *slot = Some(line); count += 1; }
        recent[..count].sort_unstable_by_key(|line| line.unwrap().sequence);
        recent[count.saturating_sub(LINE_COUNT)..count].iter().map(|line| line.unwrap().text.clone()).collect()
    }
}

#[derive(Default)]
struct Line { sequence: u64, text: String, truncated: bool }

impl Line {
    fn append(&mut self, character: char) {
        if self.truncated { return; }
        if self.text.len() + character.len_utf8() <= LINE_BYTES {
            self.text.push(character);
        } else {
            while self.text.len() > LINE_BYTES - '…'.len_utf8() { self.text.pop(); }
            self.text.push('…');
            self.truncated = true;
        }
    }
}

#[derive(Default)]
struct LineReader {
    pending: Line,
    carry: [u8; 4],
    carry_len: usize,
    escape: Escape,
}

impl LineReader {
    fn decode(&mut self, mut bytes: &[u8]) -> bool {
        let mut changed = false;
        while self.carry_len != 0 && !bytes.is_empty() {
            self.carry[self.carry_len] = bytes[0];
            match std::str::from_utf8(&self.carry[..self.carry_len + 1]) {
                Ok(text) => {
                    let character = text.chars().next().expect("carried UTF-8 character");
                    changed |= self.character(character);
                    self.carry_len = 0;
                    bytes = &bytes[1..];
                }
                Err(error) if error.error_len().is_none() => {
                    self.carry_len += 1;
                    bytes = &bytes[1..];
                }
                Err(_) => changed |= self.flush_utf8(), // Reprocess the first non-continuation byte.
            }
        }
        while !bytes.is_empty() {
            match std::str::from_utf8(bytes) {
                Ok(text) => { changed |= self.text(text); break; }
                Err(error) => {
                    let (valid, rest) = bytes.split_at(error.valid_up_to());
                    changed |= self.text(std::str::from_utf8(valid).expect("valid UTF-8 prefix"));
                    if let Some(length) = error.error_len() {
                        changed |= self.character('\u{fffd}');
                        bytes = &rest[length..];
                    } else {
                        self.carry[..rest.len()].copy_from_slice(rest);
                        self.carry_len = rest.len();
                        break;
                    }
                }
            }
        }
        changed
    }

    fn flush_utf8(&mut self) -> bool {
        if std::mem::take(&mut self.carry_len) == 0 { return false; }
        self.character('\u{fffd}')
    }

    fn text(&mut self, text: &str) -> bool {
        let mut changed = false;
        for character in text.chars() { changed |= self.character(character); }
        changed
    }

    fn character(&mut self, character: char) -> bool {
        if let Some(character) = self.escape.visible(character) {
            self.pending.append(character);
            true
        } else { false }
    }
}

// Strip ANSI/VT sequences including OSC hyperlinks and titles. Never pass C0,
// C1, bidi overrides or Unicode line/paragraph separators into native output.
#[derive(Clone, Copy, Default)]
enum Escape { #[default] Text, Esc, Intermediate, Csi, String, StringEsc }

impl Escape {
    fn visible(&mut self, character: char) -> Option<char> {
        *self = match (*self, character) {
            (Self::String | Self::StringEsc, '\u{07}' | '\u{9c}') | (Self::StringEsc, '\\') => Self::Text,
            (Self::String | Self::StringEsc, '\x1b') => Self::StringEsc,
            (Self::String | Self::StringEsc, _) => Self::String,
            (_, '\x1b') => Self::Esc,
            (Self::Esc, '[') | (_, '\u{9b}') => Self::Csi,
            (Self::Esc, ']' | 'P' | 'X' | '^' | '_') | (_, '\u{90}' | '\u{98}' | '\u{9d}'..='\u{9f}') => Self::String,
            (Self::Esc | Self::Intermediate, ' '..='/') => Self::Intermediate,
            (Self::Esc | Self::Intermediate, '0'..='~') | (Self::Csi, '@'..='~') => Self::Text,
            (Self::Csi, _) => Self::Csi,
            (_, '\t') => { *self = Self::Text; return Some(' '); }
            (_, c) if c.is_control() || matches!(c, '\u{061c}' | '\u{200e}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{206f}') => Self::Text,
            (_, c) => { *self = Self::Text; return Some(c); }
        };
        None
    }
}

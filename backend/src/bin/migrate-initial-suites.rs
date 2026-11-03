//! Explicit offline cutover only; normal startup never migrates user Tasks.
use std::path::PathBuf;
use anyhow::{Context, ensure};
use herdr_bot::files::{FileAccess, FileStore, PublishMode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Change { path: PathBuf, before: Option<Vec<u8>>, after: Vec<u8> }
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan { targets: Vec<PathBuf>, inputs: Vec<Vec<u8>>, changes: Vec<Change> }
fn optional(files: &FileAccess, path: &PathBuf) -> anyhow::Result<Option<Vec<u8>>> {
    match files.read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn main() -> anyhow::Result<()> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    ensure!(args.len() == 5, "Usage: migrate-initial-suites <tasks.json> <suites.json> <mcp.json> <exclusive-state-directory> <release-directory>\nStop ALL backend processes first. Explicit operator approval required. Never targets user data during tests. Atomic per file, not a transaction; an interrupted attempt retains its exact plan and refuses conflicting edits. No rollback. Task, Bot, Chat and Run identities are not rebuilt.");
    let args = args.iter().map(std::path::absolute).collect::<std::io::Result<Vec<_>>>()?;
    let targets = args[..3].to_vec();
    ensure!(targets.iter().enumerate().all(|(i, path)| targets[..i].iter().all(|other| path != other)), "targets must be distinct");
    ensure!(targets.iter().all(|path| !path.starts_with(&args[3]) && !path.starts_with(&args[4])), "targets must be outside state and release directories");
    let mut scopes = args.clone();
    scopes.extend(targets.iter().filter_map(|path| path.parent().map(ToOwned::to_owned)));
    FileStore.with(&scopes, |files| -> anyhow::Result<()> {
        let inputs = ["mapping.json", "suites.json", "mcp.json"].iter().map(|name| files.read(args[4].join(name))).collect::<std::io::Result<Vec<_>>>()?;
        let complete = args[3].join("complete.json");
        let journal = args[3].join("plan.json");
        if let Some(bytes) = optional(files, &complete)? {
            let completed: Plan = serde_json::from_slice(&bytes)?;
            ensure!(completed.targets == targets && completed.inputs == inputs, "completed migration belongs to different targets or release inputs");
            println!("Already complete; current user edits were not read or overwritten.");
            return Ok(());
        }
        let plan: Plan = if let Some(bytes) = optional(files, &journal)? {
            let plan: Plan = serde_json::from_slice(&bytes)?;
            ensure!(plan.targets == targets && plan.inputs == inputs, "pending migration belongs to different targets or release inputs");
            plan
        } else {
            let mapping: Value = serde_json::from_slice(&inputs[0])?;
            ensure!(mapping["status"] == "approved-not-applied" && mapping["task_count"] == 22, "expected approved 22-Task mapping");
            let entries = mapping["mapping"].as_array().context("mapping must contain an array")?;
            ensure!(entries.len() == 22, "expected 22 mappings");
            let before = files.read(&targets[0]).context("Task authority must exist; no Tasks are created")?;
            let mut tasks: Value = serde_json::from_slice(&before)?;
            let records = tasks["tasks"].as_array_mut().context("Task authority must contain tasks array")?;
            let mut seen = std::collections::HashSet::new();
            for entry in entries {
                let id = entry["task_id"].as_str().context("mapping lacks stable ID")?;
                ensure!(seen.insert(id), "duplicate mapping ID {id}");
                let mut matches = records.iter_mut().filter(|task| task["id"] == id);
                let task = matches.next().with_context(|| format!("conflict: current Task {id} is missing"))?;
                ensure!(matches.next().is_none(), "conflict: duplicate current Task {id}");
                ensure!(task["agent"] == entry["agent"] && task.get("suite").is_none() && task["extensions"] == entry["previous_optional_extensions"], "conflict: current environment changed for Task {id}; no edits overwritten");
                let object = task.as_object_mut().context("Task must be object")?;
                object.remove("extensions");
                object.insert("suite".into(), entry["suite_id"].clone());
            }
            for task in records.iter() {
                ensure!(task.get("extensions").is_none(), "unmapped legacy Task {}; provide an approved mapping before cutover", task["id"]);
            }
            let mut changes = Vec::new();
            // Validate all files before publishing anything. Existing unrelated
            // definitions survive; same-ID edits are explicit conflicts.
            for (target, input, key, array) in [(&targets[1], &inputs[1], "suites", true), (&targets[2], &inputs[2], "mcpServers", false)] {
                let before = optional(files, target)?;
                let approved: Value = serde_json::from_slice(input)?;
                let mut current: Value = match &before { Some(bytes) => serde_json::from_slice(bytes)?, None => if array { json!({"suites":[]}) } else { json!({"mcpServers":{}}) } };
                if array {
                    let values = current[key].as_array_mut().context("Suite authority must contain suites array")?;
                    for value in approved[key].as_array().context("release suites missing")? {
                        let mut matches = values.iter().filter(|old| old["id"] == value["id"]);
                        let existing = matches.next();
                        ensure!(matches.next().is_none() && existing.is_none_or(|old| old == value), "conflict: Suite {} already edited", value["id"]);
                        if existing.is_none() { values.push(value.clone()); }
                    }
                } else {
                    let values = current[key].as_object_mut().context("MCP authority must contain mcpServers object")?;
                    for (id, value) in approved[key].as_object().context("release MCP missing")? {
                        ensure!(values.get(id).is_none_or(|old| old == value), "conflict: MCP {id} already edited");
                        values.insert(id.clone(), value.clone());
                    }
                }
                changes.push(Change { path: target.clone(), before, after: serde_json::to_vec_pretty(&current)? });
            }
            changes.push(Change { path: targets[0].clone(), before: Some(before), after: serde_json::to_vec_pretty(&tasks)? });
            let plan = Plan { targets: targets.clone(), inputs, changes };
            files.create_dir_all(&args[3])?;
            files.publish_checked(&journal, &serde_json::to_vec_pretty(&plan)?, PublishMode::Create, |_| Ok::<_, std::io::Error>(()), |error| error)?;
            plan
        };
        for change in &plan.changes {
            let current = optional(files, &change.path)?;
            if current.as_ref() == Some(&change.after) { continue; }
            ensure!(current == change.before, "conflict after partial migration: {}; completed files retained, no rollback or overwriting edits", change.path.display());
            if let Some(parent) = change.path.parent() { files.create_dir_all(parent)?; }
            files.publish_checked(&change.path, &change.after, if change.before.is_some() { PublishMode::Replace } else { PublishMode::Create }, |files| {
                ensure!(optional(files, &change.path)? == change.before, "conflict immediately before publication: {}", change.path.display());
                Ok::<_, anyhow::Error>(())
            }, anyhow::Error::from).with_context(|| format!("partial migration failed at {}; successful earlier files retained; completion not recorded", change.path.display()))?;
        }
        // Independent readback, not successful-write counts, authorizes completion.
        for change in &plan.changes {
            ensure!(files.read(&change.path)? == change.after, "readback conflict: {}", change.path.display());
        }
        files.publish_checked(&complete, &serde_json::to_vec_pretty(&plan)?, PublishMode::Create, |_| Ok::<_, std::io::Error>(()), |error| error)?;
        println!("Complete: 22 stable Task IDs migrated; retained plan records original bytes. Bots, history and Runs were not opened.");
        Ok(())
    })??;
    Ok(())
}

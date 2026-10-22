//! 受控的 Herdr CLI 替身：只在自动化验收中使用，不是生产 adapter。
//!
//! 它模仿 herdr CLI 的命令行形状与新行分隔 JSON 响应，并按自己的可执行文件所在目录读取
//! `workspaces.json` / `behavior.json`、写下 `calls.jsonl` 与 `tabs.jsonl`。测试把本二进制复制
//! 到独立临时目录后，把 `[herdr] executable` 指向副本：于是 herdr-bot 走的是**同一套生产调用
//! 包装层**（逐参数启动、无 Shell），而外部记录来自这个独立进程，不由 herdr-bot 的计算路径生成。
//!
//! 支持的命令与行为：
//! - `workspace list`
//! - `tab create --workspace ID --cwd PATH --label TEXT --no-focus`
//! - `agent start NAME --kind KIND --pane ID --timeout MS [-- ARGS...]`
//! - `agent prompt TARGET TEXT`
//!
//! `behavior.json` 可延迟或拒绝某一步，用来观察“启动中”“提交中”与失败边界：
//! `workspace_list_delay_ms`、`tab_create_delay_ms`、`agent_start_delay_ms`、`prompt_delay_ms`，
//! 以及布尔值 `fail_workspace_list`、`fail_tab_create`、`fail_agent_start`、`fail_prompt`。
//!
//! 空文本提交与真实 Herdr 一样以 `empty_agent_prompt` 拒绝；拒绝前仍写下调用记录，因为
//! “herdr-bot 尝试提交什么”与“Herdr 是否接受”是两件需要分别观察的事实。
//!
//! 并发用例需要“只拦住其中一次创建”的受控屏障，因此还有三个可选门：
//! `tab_create_gate`（匹配 Tab 名称）、`agent_start_gate` 与 `prompt_gate`，各自是
//! `{ "match": 子串, "wait_for_file": 路径, "timeout_ms": 毫秒 }`。调用先写入记录，再在匹配时
//! 等待释放文件出现（或到超时）才返回；`match` 省略或为空表示不筛选，`timeout_ms` 缺省 30 秒。
//! 释放文件由用例创建，因此“一个创建受阻、另一个继续”来自外部进程的可控停顿，不是固定延时。

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("workspace") if args.get(1).map(String::as_str) == Some("list") => {
            workspace_list(&args)
        }
        Some("tab") if args.get(1).map(String::as_str) == Some("create") => tab_create(&args),
        Some("agent") if args.get(1).map(String::as_str) == Some("start") => agent_start(&args),
        Some("agent") if args.get(1).map(String::as_str) == Some("prompt") => agent_prompt(&args),
        _ => usage(&args),
    }
}

/// 数据目录：优先环境变量，否则使用本可执行文件所在目录（测试复制出的独立副本）。
fn data_dir() -> PathBuf {
    match env::var("HERDR_FIXTURE_DIR") {
        Ok(directory) => PathBuf::from(directory),
        Err(_) => env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(".")),
    }
}

fn behavior() -> Value {
    fs::read_to_string(data_dir().join("behavior.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| json!({}))
}

fn delay(behavior: &Value, key: &str) {
    let millis = behavior.get(key).and_then(Value::as_u64).unwrap_or(0);
    if millis > 0 {
        thread::sleep(std::time::Duration::from_millis(millis));
    }
}

fn flag(behavior: &Value, key: &str) -> bool {
    behavior.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// 受控屏障：调用匹配 `match` 时，一直等到释放文件出现才返回。
///
/// 记录已经在等待之前写下，所以用例能先看见“这一步已被调用”，再确认另一条创建没有被它挡住。
/// 等到 `timeout_ms` 仍未释放时按超时继续，让用例自己的时限给出失败，而不是让夹具永久挂住。
fn wait_at_gate(behavior: &Value, key: &str, subject: &str) {
    let Some(gate) = behavior.get(key) else {
        return;
    };
    if let Some(pattern) = gate.get("match").and_then(Value::as_str) {
        if !pattern.is_empty() && !subject.contains(pattern) {
            return;
        }
    }
    let Some(file) = gate.get("wait_for_file").and_then(Value::as_str) else {
        return;
    };
    let path = PathBuf::from(file);
    let path = if path.is_absolute() {
        path
    } else {
        data_dir().join(path)
    };
    let timeout = gate
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(30_000);
    let deadline = Instant::now() + Duration::from_millis(timeout);
    while !path.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
}

/// 追加一条外部调用记录；测试只读这个文件，不读 herdr-bot 的进程内状态。
fn record(file: &str, entry: Value) {
    let path = data_dir().join(file);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("打开夹具记录文件");
    // 一次写完整行，避免读取方在轮询时看到半行 JSON。
    let line = format!("{entry}\n");
    file.write_all(line.as_bytes()).expect("写入夹具记录");
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0)
}

fn unique_suffix() -> String {
    let counter = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    format!(
        "{:x}",
        (now ^ counter.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ u64::from(process::id())) & 0xff_ffff
    )
}

fn workspace_list(args: &[String]) {
    let behavior = behavior();
    record(
        "calls.jsonl",
        json!({
            "at": now_ms(),
            "kind": "workspace_list",
            "argv": args,
        }),
    );
    delay(&behavior, "workspace_list_delay_ms");
    if flag(&behavior, "fail_workspace_list") {
        fail("workspace_list_failed", "受控夹具拒绝列出 Workspace");
    }

    let workspaces: Value = fs::read_to_string(data_dir().join("workspaces.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| json!([]));
    output(json!({
        "id": "cli:workspace:list",
        "result": {
            "type": "workspace_list",
            "workspaces": workspaces,
        },
    }));
}

fn tab_create(args: &[String]) {
    let behavior = behavior();
    let workspace = flag_value(args, "--workspace").unwrap_or_default();
    let cwd = flag_value(args, "--cwd").unwrap_or_default();
    let label = flag_value(args, "--label").unwrap_or_default();
    record(
        "calls.jsonl",
        json!({
            "at": now_ms(),
            "kind": "tab_create",
            "argv": args,
            "workspace": workspace,
            "cwd": cwd,
            "label": label,
        }),
    );
    delay(&behavior, "tab_create_delay_ms");
    wait_at_gate(&behavior, "tab_create_gate", &label);
    if flag(&behavior, "fail_tab_create") {
        fail("tab_create_failed", "受控夹具拒绝新建 Tab");
    }

    let suffix = unique_suffix();
    let tab_id = format!("{workspace}:t{suffix}");
    let pane_id = format!("{workspace}:p{suffix}");
    record(
        "tabs.jsonl",
        json!({
            "at": now_ms(),
            "workspace": workspace,
            "cwd": cwd,
            "label": label,
            "tab_id": tab_id,
            "pane_id": pane_id,
        }),
    );
    output(json!({
        "id": "cli:tab:create",
        "result": {
            "type": "tab_created",
            "tab": {
                "tab_id": tab_id,
                "workspace_id": workspace,
                "number": 1,
                "label": label,
                "focused": false,
                "pane_count": 1,
                "agent_status": "unknown",
            },
            "root_pane": {
                "pane_id": pane_id,
                "tab_id": tab_id,
                "workspace_id": workspace,
                "cwd": cwd,
                "label": Value::Null,
                "agent": Value::Null,
                "agent_status": "unknown",
                "focused": false,
                "revision": 1,
                "state_labels": {},
            },
        },
    }));
}

fn agent_start(args: &[String]) {
    let behavior = behavior();
    let Some(name) = args.get(2).cloned() else {
        usage(args);
    };
    let kind = flag_value(args, "--kind").unwrap_or_default();
    let pane = flag_value(args, "--pane").unwrap_or_default();
    let extra: Vec<String> = match args.iter().position(|arg| arg == "--") {
        Some(index) => args[index + 1..].to_vec(),
        None => Vec::new(),
    };
    record(
        "calls.jsonl",
        json!({
            "at": now_ms(),
            "kind": "agent_start",
            "argv": args,
            "name": name,
            "agent_kind": kind,
            "pane": pane,
            "args": extra,
        }),
    );
    delay(&behavior, "agent_start_delay_ms");
    wait_at_gate(&behavior, "agent_start_gate", &args.join(" "));
    if flag(&behavior, "fail_agent_start") {
        fail("agent_start_failed", "受控夹具拒绝启动 agent");
    }

    let workspace = pane.split(':').next().unwrap_or_default();
    output(json!({
        "id": "cli:agent:start",
        "result": {
            "type": "agent_started",
            "agent": {
                "terminal_id": pane,
                "name": name,
                "agent": kind,
                "agent_status": "idle",
                "interactive_ready": true,
                "workspace_id": workspace,
                "tab_id": format!("{workspace}:t0"),
                "pane_id": pane,
                "focused": false,
                "revision": 1,
            },
            "argv": extra,
        },
    }));
}

fn agent_prompt(args: &[String]) {
    let behavior = behavior();
    let Some(target) = args.get(2).cloned() else {
        usage(args);
    };
    let text = args.get(3).cloned().unwrap_or_default();
    record(
        "calls.jsonl",
        json!({
            "at": now_ms(),
            "kind": "agent_prompt",
            "argv": args,
            "target": target,
            "text": text,
        }),
    );
    delay(&behavior, "prompt_delay_ms");
    wait_at_gate(&behavior, "prompt_gate", &text);
    if text.is_empty() {
        // 真实 Herdr 在写入 pane 之前就以 `empty_agent_prompt` 拒绝空文本；受控替身保持同一结果。
        // 调用记录已经写下，因此测试能够证明 herdr-bot 确实尝试过这次提交，而不是跳过它。
        fail("empty_agent_prompt", "agent prompt must not be empty");
    }
    if flag(&behavior, "fail_prompt") {
        fail("agent_prompt_failed", "受控夹具拒绝提交初始指示");
    }

    output(json!({
        "id": "cli:agent:prompt",
        "result": {
            "type": "agent_prompted",
            "agent": {
                "terminal_id": target,
                "name": target,
                "agent": "omp",
                "agent_status": "working",
                "interactive_ready": true,
                "workspace_id": target.split(':').next().unwrap_or_default(),
                "tab_id": "fixture:t0",
                "pane_id": target,
                "focused": false,
                "revision": 2,
            },
        },
    }));
}

/// 取 `--flag value` 的值；同名参数取第一次出现。
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1).cloned())
}

fn output(value: Value) {
    println!("{value}");
}

fn fail(code: &str, message: &str) -> ! {
    eprintln!(
        "{}",
        json!({
            "id": "cli:fixture",
            "error": { "code": code, "message": message },
        })
    );
    process::exit(1);
}

fn usage(args: &[String]) -> ! {
    // 未预期的命令也记入外部调用记录：测试据此证明失败或超时之后没有额外的
    // 删除、关闭、重建或重发动作，而不是只相信 herdr-bot 自己返回的结果。
    record(
        "calls.jsonl",
        json!({
            "at": now_ms(),
            "kind": "unsupported",
            "argv": args,
        }),
    );
    eprintln!(
        "{}",
        json!({
            "id": "cli:fixture",
            "error": {
                "code": "unsupported_command",
                "message": format!("受控 Herdr 夹具不认识这些参数：{args:?}"),
            },
        })
    );
    process::exit(2);
}

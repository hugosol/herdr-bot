//! 集成测试共用的支持设施：复制仓库内的验证夹具、读取夹具调用记录、判断进程存活。
//!
//! 每个测试都把仓库 `fixtures/` 目录复制到独立临时目录后再驱动，于是夹具记录目录天然隔离：
//! 并行运行的测试、其他 ticket 的测试、以及人工运行都不会互相污染同一份调用记录。
//!
//! 夹具的调用记录格式（制表符分隔，见 `fixtures/README.md`）：
//! `call	seq	timestamp	fixture	stage	callid	pid	cwd	detail`

#![allow(dead_code)]

pub mod sse;

use std::fs;
use std::net::SocketAddr;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;

/// 记录文件相对夹具根目录的位置。
pub const RECORD_FILE: &str = "runtime/calls.tsv";

/// 独立临时目录；路径中带空格，覆盖 Windows 命令行引号处理的真实情况。
pub struct TempDir(PathBuf);

static COUNTER: AtomicU64 = AtomicU64::new(0);

impl TempDir {
    pub fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join("omp bot fixture checks")
            .join(format!("{label} {}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("创建临时目录");
        TempDir(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 一次测试独占的夹具副本：仓库夹具被完整复制到临时目录后才有记录写入。
pub struct FixtureTree {
    root: TempDir,
}

impl FixtureTree {
    /// 复制仓库 `fixtures/` 的内容到独立临时目录。
    pub fn new(label: &str) -> Self {
        let root = TempDir::new(label);
        copy_dir(&repo_fixtures_dir(), root.path());
        FixtureTree { root }
    }

    /// 副本根目录（等同于仓库 `fixtures/` 的根）。
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// 某个操作夹具批处理的绝对路径。
    pub fn script(&self, name: &str) -> PathBuf {
        self.root().join("operations").join(name)
    }

    /// 被配置为工作目录的目录。
    pub fn operations(&self) -> PathBuf {
        self.root().join("operations")
    }

    /// 记录目录（夹具默认写入位置）。
    pub fn runtime(&self) -> PathBuf {
        self.root().join("runtime")
    }

    /// 记录文件路径。
    pub fn record_file(&self) -> PathBuf {
        self.root().join(RECORD_FILE)
    }

    /// 已写入的调用记录。
    pub fn records(&self) -> Vec<CallRecord> {
        read_records(&self.root().join(RECORD_FILE))
    }

    /// 人工结束入口：夹具目录内的结束脚本（与人工结束时用的是同一个脚本）。
    pub fn stop_fixtures(&self) -> std::process::Output {
        run_batch_blocking(&self.root().join("stop-fixtures.bat"), self.root(), &[])
    }

    /// 同一个人工结束入口，但显式指定等待夹具自行结束的宽限秒数（`stop-fixtures.bat` 原样转发参数）。
    ///
    /// 夹具每 200ms 轮询一次停止标记；满载（并行测试）时轮询可能被推迟，宽限给足才能让
    /// “夹具按停止标记自行结束”保持为可观察结果，而不是落到结束脚本的兜底强杀。
    pub fn stop_fixtures_with_grace(&self, grace_seconds: u32) -> std::process::Output {
        run_batch_blocking_args(
            &self.root().join("stop-fixtures.bat"),
            self.root(),
            &["-GraceSeconds", &grace_seconds.to_string()],
            &[],
        )
    }
}

impl Drop for FixtureTree {
    fn drop(&mut self) {
        // 测试失败也不会遗留运行中的夹具进程。
        let _ = self.stop_fixtures();
    }
}

/// 调用记录的一行。
#[derive(Debug, Clone)]
pub struct CallRecord {
    /// 调用序号：同一夹具的第几次调用（从 1 开始）。
    pub call: u64,
    /// 记录行序号：记录文件内的全局递增行号（从 1 开始，无缺口）。
    pub seq: u64,
    /// 调用时间（ISO 8601，含毫秒与本地时区偏移）。
    pub timestamp: String,
    /// 夹具标识。
    pub fixture: String,
    /// 记录阶段：`invoke` 表示一次真实调用开始。
    pub stage: String,
    /// 同一次调用的稳定标识；同一进程写出的各阶段记录相同。
    pub call_id: String,
    /// 夹具进程 PID。
    pub pid: u32,
    /// 夹具进程当时的实际工作目录。
    pub cwd: PathBuf,
    /// 附加说明。
    pub detail: String,
}

/// 读取记录文件；文件不存在（尚无任何调用）时返回空列表。
pub fn read_records(path: &Path) -> Vec<CallRecord> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_record(path, line))
        .collect()
}

fn parse_record(path: &Path, line: &str) -> CallRecord {
    let fields: Vec<&str> = line.split('\t').collect();
    assert_eq!(
        fields.len(),
        9,
        "记录行必须是 9 个制表符分隔字段（{}）：{line}",
        path.display()
    );
    let parse_number = |index: usize, name: &str| {
        fields[index]
            .parse()
            .unwrap_or_else(|_| panic!("记录字段 {name} 必须是数字：{line}"))
    };
    CallRecord {
        call: parse_number(0, "call"),
        seq: parse_number(1, "seq"),
        timestamp: fields[2].to_string(),
        fixture: fields[3].to_string(),
        stage: fields[4].to_string(),
        call_id: fields[5].to_string(),
        pid: parse_number(6, "pid") as u32,
        cwd: PathBuf::from(fields[7]),
        detail: fields[8].to_string(),
    }
}

/// 仓库内验证夹具的根目录。
pub fn repo_fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
}

/// 递归复制目录内容。
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|e| panic!("创建目录失败 {}：{e}", to.display()));
    for entry in fs::read_dir(from).unwrap_or_else(|e| panic!("读取目录失败 {}：{e}", from.display()))
    {
        let entry = entry.expect("读取目录项");
        let name = entry.file_name();
        // `runtime/` 只存放运行时记录，不参与复制。
        if name == "runtime" {
            continue;
        }
        let target = to.join(&name);
        if entry.file_type().expect("目录项类型").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("复制夹具文件");
        }
    }
}

/// 服务端配置：把某个夹具批处理绑定为“启动”操作，并使用配置的工作目录。
///
/// 只驱动“启动”的测试不需要区分两个操作，因此“停止”先绑定同一个目标；
/// 需要两个不同目标的测试自己改写 `operations.stop`。关机倒计时取配置缺省的 10 秒；
/// 需要驱动实际到期的用例用 [`config_with_countdown`]。
pub fn config(script: &Path, working_directory: &Path) -> omp_bot::Config {
    config_with_countdown(script, working_directory, 10)
}

/// 与 [`config`] 同形，但显式指定关机倒计时秒数（驱动到期用例用它把等待压到最短的合法值）。
pub fn config_with_countdown(
    script: &Path,
    working_directory: &Path,
    countdown_seconds: u32,
) -> omp_bot::Config {
    let binding = || omp_bot::OperationBinding {
        script: script.to_path_buf(),
        working_directory: working_directory.to_path_buf(),
    };
    omp_bot::Config {
        listen_addr: "0.0.0.0:0".to_string(),
        operations: omp_bot::Operations {
            start: binding(),
            stop: binding(),
        },
        mdns: omp_bot::MdnsConfig::default(),
        shutdown: omp_bot::ShutdownConfig {
            countdown_seconds: omp_bot::CountdownSeconds::new(countdown_seconds)
                .expect("测试用的倒计时秒数必须合法"),
        },
        // 会话窗口在这些用例里不是受测对象：取缺省（在线 10 秒、离线保留 30 秒）。
        // 需要驱动配置窗口的用例自己解析 TOML 并注入策略（见 `session_window_config.rs`）。
        sessions: omp_bot::SessionsConfig::default(),
        // workspace 登记文件同样不是这些用例的受测对象；需要隔离存储的用例自己改写这个路径。
        workspaces: omp_bot::WorkspacesConfig::default(),
    }
}

/// 经 HTTP interface 请求一个固定操作：裸 POST，不带请求体或查询串。
pub async fn post_operation(
    app: axum::Router,
    name: &str,
) -> (axum::http::StatusCode, serde_json::Value) {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/operations/{name}"))
        .body(Body::empty())
        .expect("构造请求");
    let response = app.oneshot(request).await.expect("请求固定操作");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("读取响应体");
    (
        status,
        serde_json::from_slice(&bytes).expect("响应体是 JSON"),
    )
}

/// 经 HTTP interface 请求“启动”操作。
pub async fn post_start(app: axum::Router) -> (axum::http::StatusCode, serde_json::Value) {
    post_operation(app, "start").await
}

/// 在临时端口上真实监听 HTTP 的测试服务器，提供与交付运行时同一份 `omp_bot::app`。
///
/// 与 `post_operation`（在进程内直接把请求交给 Router）不同，这里的每个请求都经独立 TCP 连接发出，
/// 因此“多个客户端同时提交”来自真正独立的 HTTP 客户端：既覆盖多页面/多设备同时提交的实际情况，
/// 也不经过任何页面与按钮状态。
pub struct TestServer {
    addr: SocketAddr,
    serving: tokio::task::JoinHandle<()>,
}

impl TestServer {
    /// 在 `127.0.0.1` 的临时端口上启动应用。
    pub async fn start(config: &omp_bot::Config) -> Self {
        Self::start_app(omp_bot::app(config)).await
    }

    /// 在 `127.0.0.1` 的临时端口上启动**给定的应用**（例如注入了记录型关机动作的应用）。
    ///
    /// 与 `start` 完全同一形态（真实监听端口 + 每个请求独立 TCP 连接），只是装配好的应用由调用者
    /// 提供；路由注册与交付运行时一致，因为两种装配都只经 `omp_bot::app` / `omp_bot::app_with_power_off`。
    pub async fn start_app(app: axum::Router) -> Self {
        Self::start_app_on("127.0.0.1:0".parse().expect("回环测试监听地址"), app).await
    }

    /// 在**指定**地址的临时端口上启动给定的应用。
    ///
    /// 用于让请求的 TCP 对端地址取到回环之外的地址：绑定本机某个网卡地址后再连它，同机访问的源
    /// 地址就是那个网卡地址——正是扩展经 `omp-bot.local` 连进来时服务端看到的样子。
    pub async fn start_app_on(bind: SocketAddr, app: axum::Router) -> Self {
        let listener = tokio::net::TcpListener::bind(bind)
            .await
            .expect("绑定测试监听端口");
        let addr = listener.local_addr().expect("读取测试监听地址");
        let serving = tokio::spawn(async move {
            // 与交付运行时同一份装配：会话事件写入的来源校验靠连接信息拿到 TCP 对端地址。
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await;
        });
        TestServer { addr, serving }
    }

    /// 监听地址，用于构造独立的 HTTP 客户端。
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.serving.abort();
    }
}

/// 一次真实 HTTP 请求的响应。
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP 状态码。
    pub status: u16,
    /// 响应头（名已转小写，用于大小写不敏感地查找）。
    pub headers: Vec<(String, String)>,
    /// 响应体。
    pub body: String,
}

/// 经一条新的 TCP 连接提交一次裸 POST：只带路径，没有查询串、没有请求体。
///
/// 这是最小的 HTTP/1.1 客户端：请求与页面无关，也不经过任何前端代码。
pub async fn post_over_tcp(addr: SocketAddr, path: &str) -> HttpResponse {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("连接测试服务器");
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .expect("发送操作请求");
    stream.flush().await.expect("发送操作请求");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("读取响应");
    parse_response(&raw)
}

/// 经一条新的 TCP 连接提交一次裸 GET：只带路径，没有查询串、没有请求体。
///
/// 与 [`post_over_tcp`] 同一形态（真实 TCP、不经过任何前端代码），用于只读列表的独立观察。
pub async fn get_over_tcp(addr: SocketAddr, path: &str) -> HttpResponse {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("连接测试服务器");
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("发送读取请求");
    stream.flush().await.expect("发送读取请求");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("读取响应");
    parse_response(&raw)
}

/// 经一条新的 TCP 连接提交一次**带 JSON 请求体**的 POST。
///
/// 与 `post_over_tcp` 同一形态（真实 TCP、不经过任何前端代码），只是请求面从空请求体换成
/// `Content-Type: application/json` 加一段请求体：会话事件接口的请求面就是一条 JSON 事件。
pub async fn post_json_over_tcp(addr: SocketAddr, path: &str, body: &str) -> HttpResponse {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("连接测试服务器");
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .await
        .expect("发送请求");
    stream.flush().await.expect("发送请求");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("读取响应");
    parse_response(&raw)
}

/// 一条已经发出操作请求、但**尚未读取任何响应**的 HTTP 连接。
///
/// 与 `post_over_tcp`（发完请求一直读到连接结束）不同，这里把连接留在调用者手里：调用者可以在
/// 脚本仍在执行时主动关闭这条 socket，从而制造“客户端中途断线”这一真实情形。关闭是本进程真实的
/// TCP 断开，不是对取消的模拟，也不经过页面、按钮状态或任何前端代码。
pub struct OpenRequest {
    stream: tokio::net::TcpStream,
}

impl OpenRequest {
    /// 经一条新的 TCP 连接提交一次裸 POST，但不等待响应。
    pub async fn submit(addr: SocketAddr, path: &str) -> Self {
        use tokio::io::AsyncWriteExt;

        let mut stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("连接测试服务器");
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .await
            .expect("发送操作请求");
        stream.flush().await.expect("发送操作请求");
        OpenRequest { stream }
    }

    /// 客户端主动中止连接：关闭 socket，不再读取属于这次请求的任何响应，
    /// 也不发任何“取消”请求——断线只是断线。
    pub fn abort(self) {
        drop(self.stream);
    }
}

fn parse_response(raw: &[u8]) -> HttpResponse {
    let head_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("响应必须包含头部结束空行")
        + 4;
    let (head, body) = raw.split_at(head_end);
    let head = String::from_utf8_lossy(head).into_owned();

    let mut lines = head.lines();
    let status_line = lines.next().expect("响应必须包含状态行");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .expect("状态行必须包含状态码")
        .parse()
        .expect("状态码必须是数字");
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| {
            line.split_once(':')
                .map(|(name, value)| (name.trim().to_lowercase(), value.trim().to_string()))
        })
        .collect();

    let chunked = headers
        .iter()
        .any(|(name, value)| name == "transfer-encoding" && value.to_lowercase().contains("chunked"));
    let body = if chunked {
        decode_chunked(body)
    } else {
        body.to_vec()
    };

    HttpResponse {
        status,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    }
}

/// 解码 `Transfer-Encoding: chunked` 响应体（Axum 只在流式响应上使用它）。
fn decode_chunked(mut rest: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::new();
    loop {
        let Some(line_end) = rest.windows(2).position(|window| window == b"\r\n") else {
            break;
        };
        let size_field = String::from_utf8_lossy(&rest[..line_end]);
        let size_field = size_field.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_field, 16).unwrap_or(0);
        rest = &rest[line_end + 2..];
        if size == 0 {
            break;
        }
        let take = size.min(rest.len());
        decoded.extend_from_slice(&rest[..take]);
        rest = if rest.len() >= take + 2 {
            &rest[take + 2..]
        } else {
            &[]
        };
    }
    decoded
}

/// `CREATE_NO_WINDOW`：与后端一致的无窗口启动标志。
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 以与后端相同的真实机制调用一个批处理：`cmd.exe /C <script>`，关闭标准输入输出并等待结束。
pub async fn run_batch(
    script: &Path,
    working_directory: &Path,
    env: &[(&str, &str)],
) -> Option<i32> {
    let mut command = tokio::process::Command::new("cmd.exe");
    command
        .arg("/C")
        .arg(script)
        .current_dir(working_directory)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    for (key, value) in env {
        command.env(key, value);
    }
    command
        .status()
        .await
        .expect("启动批处理")
        .code()
}

/// 同步版本，用于测试清理路径（`Drop` 中不能 await）。
pub fn run_batch_blocking(
    script: &Path,
    working_directory: &Path,
    env: &[(&str, &str)],
) -> std::process::Output {
    run_batch_blocking_args(script, working_directory, &[], env)
}

/// 同步版本，并允许向批处理追加参数（`stop-fixtures.bat` 用它把宽限秒数转发给结束脚本）。
pub fn run_batch_blocking_args(
    script: &Path,
    working_directory: &Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> std::process::Output {
    let mut command = std::process::Command::new("cmd.exe");
    command
        .arg("/C")
        .arg(script)
        .args(args)
        .current_dir(working_directory)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW);
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("运行批处理")
}

/// 进程是否仍在运行（`tasklist` 是 Windows 上不依赖夹具自身的观测方式）。
pub fn process_is_alive(pid: u32) -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output()
        .expect("运行 tasklist");
    String::from_utf8_lossy(&output.stdout).contains(&format!(",\"{pid}\","))
}

/// 等待条件成立，超时返回 `false`。
pub async fn wait_until<F>(timeout: Duration, mut condition: F) -> bool
where
    F: FnMut() -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 路径比较：忽略大小写、分隔符与结尾反斜杠差异。
pub fn same_path(left: &Path, right: &Path) -> bool {
    let normalize = |path: &Path| {
        path.display()
            .to_string()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    normalize(left) == normalize(right)
}

//! 手动关机 → 后端权威倒计时（契约 P19、P21、P23、P27、P28、P31、P32、P33、P34、P35、P36、P37、
//! P38、P40、P41、P42、P43、P45、P46、P47、P49、P50；ADR 0005）。
//!
//! 被验收的是 S2（后端应用 interface）：两条关机命令（`POST /api/shutdown/start`、`POST /api/shutdown/cancel`）
//! 的接受与拒绝类别、启动期配置决定的实际到期输出、后端对「取消 vs 到期」的唯一裁定、最近结果的可读性，
//! 以及共享观察流（`GET /api/sessions/events`）的首帧与关机完整状态事件。
//!
//! **绝不真的关机**：全部用例都经 `herdr_bot::app_with_power_off` 注入记录型关机动作（既有接缝的替身），
//! 它只记录调用次数并按用例指定返回成功、失败或停在「执行已开始」；不接触任何系统调用。真实 Win32 动作
//! 只在 `src/shutdown.rs` 的非关单元测试与一次性人工验收里出现。
//!
//! 命令与观察都走真 TCP：用例不读任何进程内状态，看到的就是页面会收到的 HTTP 响应与 SSE 帧。

mod common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::StatusCode;
use common::sse::{SseConnection, SseFrame};
use common::{FixtureTree, HttpResponse, TestServer, post_json_over_tcp, post_over_tcp};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tower::ServiceExt;

/// 开始意图的请求路径。
const START: &str = "/api/shutdown/start";
/// 全局取消的请求路径。
const CANCEL: &str = "/api/shutdown/cancel";
/// 旧的即时关机路径：迁移后不再存在，也没有别名。
const LEGACY_IMMEDIATE: &str = "/api/shutdown";

/// 记录型关机动作的失败详情：**只允许出现在诊断输出里**，因此用例断言响应体与观察帧里绝不出现它。
const FAILURE_DETAIL: &str = "recording-power-off-detail-canary-7f3a";

/// 记录型关机动作的结果：成功（关机已发出）或失败（附一段只该进诊断输出的详情）。
#[derive(Debug, Clone, Copy)]
enum RecordingOutcome {
    /// 关机已发出。
    Issued,
    /// 动作失败，附诊断输出里才有的详情。
    Fails(&'static str),
}

/// 「执行已开始」的闸门：让用例稳定地停在执行已经开始、尚未返回的那个阶段上。
///
/// 它不模拟关机，只是把记录型替身的返回时机交给用例；替身的等待是有界的（见 [`Gate::WAIT_LIMIT`]），
/// 因此用例即使失败也不会把测试进程永久挂住。
struct Gate {
    /// 执行开始时通知用例：此刻阶段已经是「执行已开始」。
    entered: mpsc::UnboundedSender<()>,
    /// 用例放行后才返回。
    released: AtomicBool,
}

impl Gate {
    /// 替身最多等多久：用例失败时也不会把工作线程永久占住。
    const WAIT_LIMIT: Duration = Duration::from_secs(60);
}

/// 记录型关机动作：记录调用次数，并按构造时指定的结果返回——**绝不**接触系统关机。
struct RecordingPowerOff {
    calls: AtomicUsize,
    outcome: RecordingOutcome,
    gate: Option<Gate>,
}

impl RecordingPowerOff {
    /// 成功返回的替身。
    fn succeeding() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            outcome: RecordingOutcome::Issued,
            gate: None,
        })
    }

    /// 失败返回的替身：失败详情只该出现在诊断输出里。
    fn failing() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            outcome: RecordingOutcome::Fails(FAILURE_DETAIL),
            gate: None,
        })
    }

    /// 成功返回、但在「执行已开始」处停下等用例放行的替身；返回的接收端在替身被调用时收到通知。
    fn gated() -> (Arc<Self>, mpsc::UnboundedReceiver<()>) {
        let (entered, receiver) = mpsc::unbounded_channel();
        let double = Arc::new(Self {
            calls: AtomicUsize::new(0),
            outcome: RecordingOutcome::Issued,
            gate: Some(Gate {
                entered,
                released: AtomicBool::new(false),
            }),
        });
        (double, receiver)
    }

    /// 到目前为止被调用了几次。
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// 放行停在「执行已开始」处的调用。
    fn release(&self) {
        if let Some(gate) = &self.gate {
            gate.released.store(true, Ordering::SeqCst);
        }
    }
}

impl PowerOff for RecordingPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            let _ = gate.entered.send(());
            let deadline = Instant::now() + Gate::WAIT_LIMIT;
            while !gate.released.load(Ordering::SeqCst) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        match self.outcome {
            RecordingOutcome::Issued => Ok(()),
            RecordingOutcome::Fails(detail) => Err(PowerOffFailure::Privilege(detail.to_string())),
        }
    }
}

/// 一个用例独占的服务：真实监听端口 + 与交付运行时同一份路由 + 记录型关机动作。
struct ShutdownServer {
    server: TestServer,
    double: Arc<RecordingPowerOff>,
}

impl ShutdownServer {
    /// 用指定的倒计时秒数装配应用（其余取测试缺省配置）。
    async fn start(countdown_seconds: u32, double: Arc<RecordingPowerOff>) -> Self {
        let config = common::config_with_countdown(
            Path::new("unused.bat"),
            Path::new("."),
            countdown_seconds,
        );
        Self::start_with_config(config, double).await
    }

    /// 用**给定配置**装配应用：缺段旧配置的用例从这里进来。
    async fn start_with_config(config: herdr_bot::Config, double: Arc<RecordingPowerOff>) -> Self {
        let power_off: Arc<dyn PowerOff> = double.clone();
        let app = herdr_bot::app_with_power_off(&config, power_off);
        Self {
            server: TestServer::start_app(app).await,
            double,
        }
    }

    fn addr(&self) -> SocketAddr {
        self.server.addr()
    }

    /// 经一条新的 TCP 连接提交一次裸 POST。
    async fn command(&self, path: &str) -> HttpResponse {
        post_over_tcp(self.addr(), path).await
    }

    /// 提交一次手动开始意图。
    async fn start_shutdown(&self) -> HttpResponse {
        self.command(START).await
    }

    /// 提交一次全局取消。
    async fn cancel_shutdown(&self) -> HttpResponse {
        self.command(CANCEL).await
    }

    /// 建一条观察连接并读掉首帧。
    async fn observe(&self) -> Observer {
        Observer::connect(self.addr()).await
    }
}

/// 一条已经读过首帧 `snapshot` 的观察连接。
struct Observer {
    connection: SseConnection,
    first: SseFrame,
}

impl Observer {
    async fn connect(addr: SocketAddr) -> Self {
        let mut connection = SseConnection::open(addr, "/api/sessions/events", &[]).await;
        let first = connection.next_frame().await;
        first.named("snapshot");
        Self { connection, first }
    }

    /// 首帧里的关机状态。
    fn first_shutdown(&self) -> Value {
        shutdown_payload(&self.first)
    }

    /// 首帧里的会话列表。
    fn first_sessions(&self) -> Vec<Value> {
        self.first.json()["sessions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    /// 取下一帧命名事件（keep-alive 跳过）。
    async fn next_named(&mut self) -> SseFrame {
        self.connection.next_named_frame().await
    }

    /// 取下一帧 `shutdown` 事件的完整关机状态。
    async fn next_shutdown(&mut self) -> Value {
        let frame = self.connection.next_event("shutdown").await;
        shutdown_payload(&frame)
    }

    /// 一直读到阶段的取值等于 `phase`，返回那一帧的关机状态。
    ///
    /// 中间读到的帧都被记下来，超时或读到别的阶段时会把它们一起报出来。
    async fn shutdown_until(&mut self, phase: &str) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let mut seen: Vec<String> = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "等待阶段 {phase:?} 超时；已观察到的帧：{seen:#?}"
            );
            let Some(frame) = self.connection.next_frame_within(remaining).await else {
                panic!("等待阶段 {phase:?} 超时；已观察到的帧：{seen:#?}");
            };
            if frame.event.as_deref() != Some("shutdown") {
                seen.push(format!("{frame:?}"));
                continue;
            }
            let view = shutdown_payload(&frame);
            let observed = view["phase"].as_str().unwrap_or_default().to_string();
            if observed == phase {
                return view;
            }
            seen.push(format!("〔{observed}〕{frame:?}"));
        }
    }

    /// 断言在给定时间内没有新的命名事件到达。
    async fn assert_quiet(&mut self, timeout: Duration) {
        self.connection.assert_no_named_frame_within(timeout).await;
    }
}

/// 一帧里的关机状态：首帧 `snapshot` 与增量 `shutdown` 用的是同一个字段名。
fn shutdown_payload(frame: &SseFrame) -> Value {
    let data = frame.json();
    let view = data
        .get("shutdown")
        .cloned()
        .unwrap_or_else(|| panic!("帧必须带 shutdown 字段：{data}"));
    assert_shutdown_shape(&view);
    view
}

/// 断言的关机状态形状：字段集合固定，缺省即省略。
fn assert_shutdown_shape(view: &Value) {
    let object = view.as_object().expect("关机状态必须是 JSON 对象");
    for key in object.keys() {
        assert!(
            [
                "monitoredSessionIds",
                "arrangementId",
                "phase",
                "deadline",
                "serverTime"
            ]
            .contains(&key.as_str()),
            "关机状态不得携带契约之外的字段 {key}：{view}"
        );
    }
    assert!(
        object["monitoredSessionIds"].is_array(),
        "监听集合必须可读（没有监听时就是空集合）：{view}"
    );
    assert!(
        matches!(
            object["phase"].as_str(),
            Some("none" | "counting-down" | "executing" | "cancelled" | "issued" | "failed")
        ),
        "阶段必须是封闭取值之一：{view}"
    );
    assert!(
        object["serverTime"].as_u64().is_some(),
        "服务端时间必须可读（前端据此对齐时钟偏差）：{view}"
    );
}

/// 断言一段原始帧文本里这些字段名按给定顺序出现（缺省省略的字段跳过，不要求出现）。
fn assert_wire_order(raw: &str, fields: &[&str]) {
    let position = |field: &str| raw.find(field);
    let mut last = None;
    for field in fields {
        let Some(at) = position(field) else {
            continue;
        };
        if let Some(previous) = last {
            assert!(previous < at, "字段顺序必须稳定（{fields:?}）：{raw}");
        }
        last = Some(at);
    }
    assert!(last.is_some(), "这些字段必须至少出现一个：{fields:?}");
}

/// 响应体里的 `shutdown` 对象。
fn shutdown_of(response: &HttpResponse) -> Value {
    let object = response_body(response);
    let view = object
        .get("shutdown")
        .cloned()
        .unwrap_or_else(|| panic!("命令回执必须带关机状态：{object}"));
    assert_shutdown_shape(&view);
    view
}

/// 响应体的 JSON。
fn response_body(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body)
        .unwrap_or_else(|error| panic!("响应体必须是 JSON（{error}）：{}", response.body))
}

/// 一次成功的开始意图的两个可观察事实：`outcome` 与关机状态。
fn assert_started(response: &HttpResponse) -> Value {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
    let outcome = response_body(response)["outcome"].clone();
    assert!(
        matches!(outcome.as_str(), Some("counting-down" | "executing")),
        "开始意图的结果类别必须是安排阶段：{}",
        response.body
    );
    if outcome == json!("counting-down") {
        assert_eq!(
            shutdown_of(response)["phase"],
            json!("counting-down"),
            "类别与状态必须一致：{}",
            response.body
        );
    }
    shutdown_of(response)
}

/// 一次成功的取消：`outcome` 固定为 `cancelled`。
fn assert_cancelled(response: &HttpResponse) -> Value {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
    assert_eq!(
        response_body(response)["outcome"],
        json!("cancelled"),
        "取消生效的类别固定：{}",
        response.body
    );
    shutdown_of(response)
}

/// 一次明确拒绝：HTTP 状态码 + 复用既有形状的 `{code, message}`。
fn assert_rejected(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(response.status, status, "响应体：{}", response.body);
    let body = response_body(response);
    assert_eq!(body["code"], code, "响应体：{}", response.body);
    assert!(
        body["message"].as_str().is_some_and(|m| !m.is_empty()),
        "拒绝必须带中文说明：{}",
        response.body
    );
    assert!(
        !response.body.contains(FAILURE_DETAIL),
        "公开结果不得混入内部系统错误详情：{}",
        response.body
    );
}

/// 等待条件成立；超时直接让用例失败。
async fn wait_until(condition: impl Fn() -> bool, timeout: Duration, what: &str) {
    assert!(
        common::wait_until(timeout, condition).await,
        "等待超时：{what}"
    );
}

/// 验收 P46：共享观察流的首帧同时包含会话与关机状态，字段与顺序固定，原会话增量语义保留。
#[tokio::test]
async fn the_first_frame_carries_sessions_and_shutdown_state_and_session_deltas_keep_their_semantics()
 {
    let server = ShutdownServer::start(10, RecordingPowerOff::succeeding()).await;
    let mut observer = server.observe().await;

    // 首帧：会话在前、关机状态在后，两者都在同一帧里。
    assert_eq!(
        observer.first.json().as_object().map(|object| object.len()),
        Some(2),
        "首帧只带会话与关机状态：{:?}",
        observer.first.raw
    );
    assert_wire_order(&observer.first.raw, &["sessions", "shutdown"]);
    assert!(observer.first_sessions().is_empty(), "还没有任何会话");

    // 关机状态的字段与顺序：监听集合、安排标识、阶段、截止时间、服务端时间。
    let first = observer.first_shutdown();
    assert_eq!(first["phase"], json!("none"), "启动时没有安排：{first}");
    assert_eq!(
        first["monitoredSessionIds"],
        json!([]),
        "没有监听时提供权威空集合：{first}"
    );
    assert!(
        first.get("arrangementId").is_none() && first.get("deadline").is_none(),
        "没有安排时这两个字段缺省即省略：{first}"
    );
    assert_wire_order(
        &observer.first.raw,
        &[
            "monitoredSessionIds",
            "arrangementId",
            "phase",
            "deadline",
            "serverTime",
        ],
    );

    // 会话增量仍是原来的语义：注册、状态变化、注销各自产生自己那一种帧。
    let register = json!({
        "sessionId": "s-manual",
        "kind": "register",
        "data": { "cwd": "/work/manual", "sessionName": "手动关机用例" }
    })
    .to_string();
    let response = post_json_over_tcp(server.addr(), "/api/sessions/events", &register).await;
    assert_eq!(response.status, 200, "会话注册应被接受：{}", response.body);
    let upsert = observer.next_named().await;
    upsert.named("upsert");
    assert_eq!(upsert.json()["session"]["sessionId"], json!("s-manual"));
    assert!(
        upsert.json().get("shutdown").is_none(),
        "会话增量不带关机状态：{:?}",
        upsert.raw
    );
    assert_eq!(
        observer.first_sessions().len(),
        0,
        "首帧已经取过，不会因为后来的增量而改变"
    );

    let unregister = json!({ "sessionId": "s-manual", "kind": "unregister" }).to_string();
    let response = post_json_over_tcp(server.addr(), "/api/sessions/events", &unregister).await;
    assert_eq!(response.status, 200, "注销应被接受：{}", response.body);
    let remove = observer.next_named().await;
    remove.named("remove");
    assert_eq!(remove.json()["sessionId"], json!("s-manual"));
}

/// 验收 P37、P46、P31：一次手动开始意图建立唯一安排，整个生命周期以完整关机状态事件下发，
/// 到期只调用一次实际关机能力。
#[tokio::test]
async fn a_manual_start_forms_one_arrangement_and_its_lifecycle_arrives_as_complete_shutdown_events()
 {
    let server = ShutdownServer::start(1, RecordingPowerOff::succeeding()).await;
    let mut observer = server.observe().await;

    let response = server.start_shutdown().await;
    let accepted = assert_started(&response);
    let arrangement = accepted["arrangementId"]
        .as_str()
        .expect("开始意图必须给出安排标识")
        .to_string();

    // 倒计时中：截止时间可读，且**不是**「指令已发出」。
    let counting = observer.next_shutdown().await;
    assert_eq!(counting["phase"], json!("counting-down"));
    assert_eq!(counting["arrangementId"], json!(arrangement));

    // 到期：先进入「执行已开始」，再给出终局；两者都是完整关机状态。
    let executing = observer.shutdown_until("executing").await;
    assert_eq!(executing["arrangementId"], json!(arrangement));
    let issued = observer.shutdown_until("issued").await;
    assert_eq!(issued["phase"], json!("issued"));
    assert_eq!(issued["arrangementId"], json!(arrangement));
    assert!(
        issued.get("deadline").is_none(),
        "终局不再有截止时间：{issued}"
    );
    assert_eq!(
        issued["monitoredSessionIds"],
        json!([]),
        "监听集合仍然可读：{issued}"
    );

    // 指令已发出不等于已经观察到断电，也不引出来第二次调用。
    assert_eq!(server.double.calls(), 1, "到期只调用一次实际关机能力");
    observer.assert_quiet(Duration::from_millis(600)).await;
    assert_eq!(server.double.calls(), 1, "静置期内不得重复调用");
}

/// 验收 P49、P38：到期执行完全由后端自己的时钟推进——没有任何浏览器／观察订阅者时也照常发生。
#[tokio::test]
async fn the_backend_executes_the_arrangement_at_its_deadline_without_any_observer() {
    let server = ShutdownServer::start(1, RecordingPowerOff::succeeding()).await;

    // 既没有观察连接，也没有额外的客户端：唯一的外部输入就是这一次开始意图。
    let response = server.start_shutdown().await;
    assert_started(&response);
    assert_eq!(server.double.calls(), 0, "截止时间之前不得调用");

    wait_until(
        || server.double.calls() == 1,
        Duration::from_secs(10),
        "到期后由后端自己调用一次实际关机能力",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(server.double.calls(), 1, "重复发起之外的路径也不得再调用");

    // 观察者后来才连上也能读到终局：最近结果保留到新安排替代或进程重启。
    let observer = server.observe().await;
    let late = observer.first_shutdown();
    assert_eq!(
        late["phase"],
        json!("issued"),
        "新调用者仍能读到结果：{late}"
    );
}

/// 验收 P23、P45、P38：启动期配置决定实际到期输出——显式 3 秒在 3 秒后调用一次，之前一次不调用。
///
/// 期望值是独立字面量 3 秒（来自配置约定），不由被测实现的公式推算。
#[tokio::test]
async fn the_configured_three_seconds_drives_the_actual_expiry_output() {
    let server = ShutdownServer::start(3, RecordingPowerOff::succeeding()).await;
    let started = Instant::now();

    let response = server.start_shutdown().await;
    let view = assert_started(&response);
    let remaining = view["deadline"].as_u64().expect("倒计时中必须有截止时间")
        - view["serverTime"].as_u64().expect("服务端时间必须可读");
    assert_eq!(
        remaining, 3_000,
        "显式配置的 3 秒必须如实成为安排时长（独立字面量）：{remaining}"
    );

    // 截止时间之前：一次都不调用。
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert_eq!(server.double.calls(), 0, "截止时间之前不得调用");

    wait_until(
        || server.double.calls() == 1,
        Duration::from_secs(10),
        "3 秒后到期调用一次",
    )
    .await;
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(2_800),
        "到期执行按配置的 3 秒推进，不得提前：{elapsed:?}"
    );
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(server.double.calls(), 1, "不得自动重试");
}

/// 验收 P23、P45：缺 `[shutdown]` 段的旧配置仍可加载，并驱动**默认 10 秒**的安排。
#[tokio::test]
async fn a_legacy_config_without_a_shutdown_section_arranges_the_default_ten_seconds() {
    let legacy = r#"
listen_addr = "0.0.0.0:0"

[operations.start]
script = 'unused.bat'
working_directory = '.'

[operations.stop]
script = 'unused.bat'
working_directory = '.'

[herdr]
workspace_name = 'bot-playground'
"#;
    let config =
        herdr_bot::Config::from_toml(legacy).expect("缺 [shutdown] 段的旧配置必须仍可加载");

    let server = ShutdownServer::start_with_config(config, RecordingPowerOff::succeeding()).await;
    let response = server.start_shutdown().await;
    let view = assert_started(&response);
    let remaining = view["deadline"].as_u64().expect("倒计时中必须有截止时间")
        - view["serverTime"].as_u64().expect("服务端时间必须可读");
    assert_eq!(
        remaining, 10_000,
        "缺段旧配置的缺省是 10 秒（独立字面量）：{remaining}"
    );

    // 收尾：取消掉这个 10 秒安排，用例不依赖它到期。
    assert_cancelled(&server.cancel_shutdown().await);
    assert_eq!(server.double.calls(), 0, "取消之后不得调用实际关机能力");
}

/// 验收 P21、P37：多个调用者交错手动发起，沿用同一安排标识与截止时间，不创建第二个安排、不延长。
#[tokio::test]
async fn interleaved_manual_starts_reuse_the_same_arrangement_and_deadline() {
    let server = ShutdownServer::start(30, RecordingPowerOff::succeeding()).await;
    let mut observer = server.observe().await;

    // 第一个调用者建立安排，记下安排标识与截止时间这两个**具体取值**。
    let first = assert_started(&server.start_shutdown().await);
    let arrangement = first["arrangementId"]
        .as_str()
        .expect("安排标识")
        .to_string();
    let deadline = first["deadline"].clone();
    assert!(
        deadline.as_u64().is_some(),
        "倒计时中必须有截止时间：{first}"
    );

    // 第二个调用者来自另一条独立连接：它必须读到同一安排，而不是新造一个。
    let second = assert_started(&server.start_shutdown().await);
    assert_eq!(
        second["arrangementId"],
        json!(arrangement),
        "不得创建第二个安排"
    );
    assert_eq!(
        second["deadline"], deadline,
        "不得重置或延长截止时间（期望值取自第一次的响应，不重算）"
    );

    // 交错再来一次，仍然一样。
    let third = assert_started(&server.start_shutdown().await);
    assert_eq!(third["arrangementId"], json!(arrangement));
    assert_eq!(third["deadline"], deadline);

    // 观察流里也只有一个安排，且没有产生任何重复的开始事件。
    let observed = observer.next_shutdown().await;
    assert_eq!(observed["arrangementId"], json!(arrangement));
    assert_eq!(observed["deadline"], deadline);
    assert_eq!(server.double.calls(), 0, "重复发起不增加调用");

    assert_cancelled(&server.cancel_shutdown().await);
}

/// 验收 P40、P32：取消在到期之前生效，越过原截止时间仍然零调用。
#[tokio::test]
async fn a_cancel_before_the_deadline_wins_and_the_original_deadline_stays_silent() {
    let server = ShutdownServer::start(1, RecordingPowerOff::succeeding()).await;
    let mut observer = server.observe().await;

    let response = server.start_shutdown().await;
    assert_started(&response);
    let counting = observer.next_shutdown().await;
    let arrangement = counting["arrangementId"].clone();

    let cancelled = assert_cancelled(&server.cancel_shutdown().await);
    assert_eq!(cancelled["phase"], json!("cancelled"));
    assert_eq!(
        cancelled["arrangementId"], arrangement,
        "取消生效要能对应到被撤销的那个安排：{cancelled}"
    );

    // 观察流也给出「取消已生效」这一终局。
    let observed = observer.shutdown_until("cancelled").await;
    assert_eq!(observed["arrangementId"], arrangement);

    // 越过原截止时间（1 秒）之后仍然没有调用。
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert_eq!(server.double.calls(), 0, "取消生效后原截止时间不得发出调用");
    observer.assert_quiet(Duration::from_millis(300)).await;
    assert_eq!(server.double.calls(), 0);
}

/// 验收 P32、P47：执行已经开始时取消被明确拒绝，同一安排不可能既取消成功又发出调用。
///
/// 多线程运行时分：记录型替身在「执行已开始」处停住等用例放行，那条等待不能把整个运行时一起卡住，
/// 否则用例根本发不出那次取消请求。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn once_the_execution_started_a_cancel_is_refused_and_never_reported_as_success() {
    let (double, mut entered) = RecordingPowerOff::gated();
    let server = ShutdownServer::start(1, Arc::clone(&double)).await;
    let mut observer = server.observe().await;

    let response = server.start_shutdown().await;
    assert_started(&response);

    // 等到执行真的开始（替身已被调用、正停在那个阶段上）：此时阶段是「执行已开始」。
    tokio::time::timeout(Duration::from_secs(15), entered.recv())
        .await
        .expect("到期必须到达实际关机能力")
        .expect("闸门通知");
    assert_eq!(double.calls(), 1, "执行开始时恰好调用一次");
    let executing = observer.shutdown_until("executing").await;
    assert!(
        executing["deadline"].as_u64().is_some(),
        "执行中的安排仍可读到它的截止时间：{executing}"
    );

    // 这个阶段的取消必须是明确拒绝，而不是「取消成功」。
    let refusal = server.cancel_shutdown().await;
    assert_rejected(&refusal, 409, "shutdown-not-cancellable");

    // 放行这次执行，让它正常走到终局。
    double.release();
    let issued = observer.shutdown_until("issued").await;
    assert_eq!(issued["arrangementId"], executing["arrangementId"]);

    // 同一个安排既没有报成取消成功，也没有被取消后仍然发出调用。
    assert_eq!(double.calls(), 1, "只有执行那一次调用");
    let settled = assert_cancelled(&server.cancel_shutdown().await);
    assert_eq!(
        settled["phase"],
        json!("issued"),
        "已经发出的安排不会因为后来的取消变成「取消成功」：{settled}"
    );
}

/// 验收 P41：延迟送达的全局取消作用于**收到请求时**的当前安排，不按旧安排标识校验。
#[tokio::test]
async fn a_late_global_cancel_applies_to_the_arrangement_current_at_receipt() {
    let server = ShutdownServer::start(60, RecordingPowerOff::succeeding()).await;
    let mut observer = server.observe().await;

    // 旧安排：一个调用者看到它，并在稍后发出取消请求。
    let old = assert_started(&server.start_shutdown().await);
    let old_id = old["arrangementId"].as_str().expect("安排标识").to_string();
    let stale_request_delivery = Duration::from_millis(300);

    // 在“延迟期间”，旧安排已经被取消，另一个调用者又建立了新安排。
    assert_cancelled(&server.cancel_shutdown().await);
    let new = assert_started(&server.start_shutdown().await);
    let new_id = new["arrangementId"].as_str().expect("安排标识").to_string();
    assert_ne!(new_id, old_id, "新安排必须是另一个安排");
    observer.shutdown_until("counting-down").await;

    // 现在才把那个“延迟的”取消请求交付给后端：它撤销的是当前安排，而不是它当初看到的旧安排。
    tokio::time::sleep(stale_request_delivery).await;
    let cancelled = assert_cancelled(&server.cancel_shutdown().await);
    assert_eq!(
        cancelled["arrangementId"],
        json!(new_id),
        "取消作用于收到请求时的当前安排：{cancelled}"
    );
    assert_eq!(cancelled["phase"], json!("cancelled"));

    // 新安排的截止时间已经过去很远（60 秒）也无所谓：取消已经生效，静置期内零调用。
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    assert_eq!(server.double.calls(), 0, "取消生效后不得发出调用");
}

/// 验收 P33、P43、P47：失败只调用一次、明确可读、不自动重试；新调用者仍能读到；内部详情不外泄。
#[tokio::test]
async fn a_failed_power_off_is_reported_once_and_stays_readable_for_new_callers() {
    let server = ShutdownServer::start(1, RecordingPowerOff::failing()).await;
    let mut observer = server.observe().await;

    let response = server.start_shutdown().await;
    assert_started(&response);
    let counting = observer.next_shutdown().await;
    let arrangement = counting["arrangementId"].clone();

    let failed = observer.shutdown_until("failed").await;
    assert_eq!(failed["arrangementId"], arrangement, "终局属于同一个安排");
    assert_eq!(server.double.calls(), 1, "失败只调用一次");

    // 等待并重新读取：仍然是同一个失败事实，没有第二次调用。
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    let reader = server.observe().await;
    let observed = reader.first_shutdown();
    assert_eq!(
        observed["phase"],
        json!("failed"),
        "新调用者仍能读到失败：{observed}"
    );
    assert_eq!(observed["arrangementId"], arrangement);
    assert_eq!(server.double.calls(), 1, "没有自动重试");

    // 公开结果里没有内部系统错误详情（它只出现在诊断输出里，由子进程用例断言）。
    assert!(
        !reader.first.raw.contains(FAILURE_DETAIL),
        "观察帧不得混入系统错误详情：{}",
        reader.first.raw
    );
}

/// 验收 P43：新安排替代旧结果，进程重启之外不保留更早的终局。
#[tokio::test]
async fn a_new_arrangement_replaces_the_previous_result() {
    let server = ShutdownServer::start(1, RecordingPowerOff::failing()).await;
    let mut observer = server.observe().await;

    assert_started(&server.start_shutdown().await);
    let failed = observer.shutdown_until("failed").await;
    let old_id = failed["arrangementId"].clone();

    // 失败之后用户可以重新手动关机：新安排替代旧结果。
    let again = assert_started(&server.start_shutdown().await);
    assert_ne!(
        again["arrangementId"], old_id,
        "新的一次手动关机建立新安排：{again}"
    );
    assert_eq!(again["phase"], json!("counting-down"));

    let observed = observer.shutdown_until("counting-down").await;
    assert_eq!(observed["arrangementId"], again["arrangementId"]);
    assert_eq!(
        observed["phase"],
        json!("counting-down"),
        "旧结果已经被新安排替代：{observed}"
    );
    assert_cancelled(&server.cancel_shutdown().await);
}

/// 验收 P37：请求面封闭，且旧的即时关机入口不再存在（没有别名）。
#[tokio::test]
async fn the_shutdown_commands_close_their_request_face_and_keep_no_immediate_alias() {
    let server = ShutdownServer::start(10, RecordingPowerOff::succeeding()).await;

    // 旧路径：不再注册，也没有即时关机别名。
    let legacy = server.command(LEGACY_IMMEDIATE).await;
    assert_eq!(
        legacy.status, 404,
        "旧的即时关机路径必须消失：{}",
        legacy.body
    );
    assert_eq!(response_body(&legacy)["code"], json!("unknown-endpoint"));

    // 带查询串或请求体的命令一律按越界请求拒绝，且不建立安排、不改变状态。
    for path in ["/api/shutdown/start?delay=0", "/api/shutdown/cancel?now=1"] {
        let response = server.command(path).await;
        assert_eq!(response.status, 400, "{path} 必须被拒绝：{}", response.body);
        assert_eq!(
            response_body(&response)["code"],
            json!("unexpected-parameters")
        );
    }
    let body_request = post_json_over_tcp(server.addr(), START, "{}").await;
    assert_eq!(
        body_request.status, 400,
        "带请求体的开始意图必须被拒绝：{}",
        body_request.body
    );

    let observer = server.observe().await;
    assert_eq!(
        observer.first_shutdown()["phase"],
        json!("none"),
        "被拒绝的请求不得建立安排"
    );
    assert_eq!(server.double.calls(), 0, "被拒绝的请求不得触发关机能力");
}

/// 验收 P37：交付装配（真实的 Win32 动作）同样注册这两条命令；探测只用 GET，不产生任何副作用。
#[tokio::test]
async fn the_production_assembly_registers_both_shutdown_commands() {
    let config = common::config(Path::new("unused.bat"), Path::new("."));
    let app = herdr_bot::app(&config);

    for path in [START, CANCEL] {
        let request = axum::http::Request::builder()
            .method("GET")
            .uri(path)
            .body(Body::empty())
            .expect("构造请求");
        let response = app.clone().oneshot(request).await.expect("请求应用");
        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{path} 必须已注册（换方法得到 405，而不是 404）"
        );
    }

    // 旧的即时关机路径在交付装配上也不存在。
    let legacy = axum::http::Request::builder()
        .method("GET")
        .uri(LEGACY_IMMEDIATE)
        .body(Body::empty())
        .expect("构造请求");
    let response = app.oneshot(legacy).await.expect("请求应用");
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "旧的即时关机路径不得在交付装配上残留"
    );
}

/// 验收 P36：固定操作持有执行资格时，关机安排照样能形成；关机不执行“停止”脚本，也不动执行资格。
#[tokio::test(flavor = "multi_thread")]
async fn an_arrangement_forms_while_a_fixed_operation_holds_the_execution_permit() {
    let tree = FixtureTree::new("manual-shutdown-permit");
    let mut config =
        common::config_with_countdown(&tree.script("long-running.bat"), &tree.operations(), 10);
    // “停止”绑定立即失败的夹具：任何“停止”调用都会留下一条记录。
    config.operations.stop.script = tree.script("failure.bat");

    let double = RecordingPowerOff::succeeding();
    let power_off: Arc<dyn PowerOff> = double.clone();
    let app = herdr_bot::app_with_power_off(&config, power_off);
    let server = TestServer::start_app(app).await;

    // 长跑夹具占住执行资格（正对照：单次“启动”确实产生一次调用记录）。
    let addr = server.addr();
    let start_request =
        tokio::spawn(async move { post_over_tcp(addr, "/api/operations/start").await });
    assert!(
        common::wait_until(Duration::from_secs(30), || !invokes(&tree, "long-running")
            .is_empty())
        .await,
        "正对照：单次“启动”请求必须真实产生一次长跑夹具调用：{:#?}",
        tree.records()
    );

    // 资格被占用时，关机安排照样能形成。
    let arranged = assert_started(&post_over_tcp(server.addr(), START).await);
    assert_eq!(arranged["phase"], json!("counting-down"));
    assert_eq!(double.calls(), 0, "安排形成不等于调用关机能力");
    assert!(
        invokes(&tree, "failure.bat").is_empty(),
        "关机不得执行“停止”脚本：{:#?}",
        tree.records()
    );

    // 取消同样可用，且仍然不动执行资格、不执行任何脚本。
    assert_cancelled(&post_over_tcp(server.addr(), CANCEL).await);
    assert!(
        invokes(&tree, "failure.bat").is_empty(),
        "取消也不得执行“停止”脚本：{:#?}",
        tree.records()
    );

    // 资格仍在原持有者手里：同期到达的“停止”请求得到既有忙碌语义。
    let busy = post_over_tcp(server.addr(), "/api/operations/stop").await;
    assert_eq!(busy.status, 409, "忙碌语义不变：{}", busy.body);

    // 结束长跑夹具，资格照常释放（关机既没有获取也没有提前释放它）。
    let stop_output = tree.stop_fixtures();
    assert!(
        stop_output.status.success(),
        "结束脚本必须成功退出：{stop_output:?}"
    );
    let ended = tokio::time::timeout(Duration::from_secs(30), start_request)
        .await
        .expect("脚本结束后正在执行的请求必须返回")
        .expect("正在执行的请求任务");
    assert_eq!(
        ended.status, 200,
        "脚本结束后“启动”请求正常返回：{}",
        ended.body
    );
    assert_eq!(double.calls(), 0, "整个过程没有调用关机能力");
}

/// 某个夹具的真实调用记录（`stage = invoke` 的行）。
fn invokes(tree: &FixtureTree, fixture: &str) -> Vec<common::CallRecord> {
    tree.records()
        .into_iter()
        .filter(|record| record.fixture == fixture && record.stage == "invoke")
        .collect()
}

/// 验收 P38、P47：诊断输出记录到期执行的 `invoked` / `failed` 事实，类别机器可断言。
///
/// 诊断行写在进程输出里（S3 诊断日志 seam），而进程内拿不到自己的输出，因此这里让**同一个测试
/// 二进制**以 `--exact --nocapture` 在子进程里跑一次真实的到期路径（记录型动作，绝不真的关机），
/// 由父进程断言子进程的原始输出里两个类别行都在、且互相区分。
#[test]
fn diagnostics_of_the_exercised_expiry_path_are_machine_assertable() {
    let exe = std::env::current_exe().expect("测试二进制路径");
    let output = std::process::Command::new(exe)
        .args([
            "--exact",
            "--nocapture",
            "manual_shutdown_diagnostics_on_the_expiry_path",
        ])
        .output()
        .expect("在子进程里运行诊断用例");
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "子进程里的诊断用例必须通过：\n{log}"
    );

    assert!(
        log.contains("[herdr-bot] 关机状态：invoked"),
        "到期成功必须留下 invoked 类别：\n{log}"
    );
    assert!(
        log.contains("[herdr-bot] 关机状态：failed step=privilege error=")
            && log.contains(FAILURE_DETAIL),
        "到期失败必须留下 failed 类别与系统原始错误（详情只出现在这里）：\n{log}"
    );
}

/// 子进程用例：让两条到期路径各走一遍真实端点路径（成功与失败，都用记录型动作）。
#[tokio::test]
async fn manual_shutdown_diagnostics_on_the_expiry_path() {
    for double in [
        RecordingPowerOff::succeeding(),
        RecordingPowerOff::failing(),
    ] {
        let server = ShutdownServer::start(1, double).await;
        assert_started(&server.start_shutdown().await);
        wait_until(
            || server.double.calls() == 1,
            Duration::from_secs(15),
            "到期调用记录型关机动作",
        )
        .await;
    }
}

//! `POST /api/shutdown` 的端点契约与关机动作接缝（契约 P7、P8；票 04）。
//!
//! 被验收的是 S2（HTTP 操作 interface）：请求面封闭（空 POST → 200；带参数 → 400
//! `unexpected-parameters` 且无副作用）、动作失败 → 500 `shutdown-failed`、与固定操作的执行资格完全
//! 解耦（脚本执行期间照样被接受，忙碌语义不变、不执行“停止”脚本、不获取也不释放资格）；诊断输出
//! （S3 诊断日志）的类别另有断言。
//!
//! **绝不真的关机**：全部用例都经 `omp_bot::app_with_power_off` 注入记录型关机动作，它只记录调用
//! 次数并按测试指定返回成功或失败，不接触任何系统调用。真实 Win32 动作只在提权测试（非关机，
//! 见 `src/shutdown.rs` 的单元测试）与一次性人工验收里出现。

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{FixtureTree, HttpResponse, TestServer, config, post_over_tcp};
use serde_json::Value;
use tokio::task::JoinHandle;
use tower::ServiceExt;

/// “启动”操作的请求路径。
const START: &str = "/api/operations/start";
/// “停止”操作的请求路径。
const STOP: &str = "/api/operations/stop";
/// 关机端点的请求路径。
const SHUTDOWN: &str = "/api/shutdown";

/// 既有忙碌语义的编码（与 `backend/tests/mutual_exclusion.rs` 一致）。
const BUSY_STATUS: u16 = 409;
const BUSY_CODE: &str = "operation-in-progress";

/// 资格释放后的静置时间：足够让“关机把请求排队、之后补做”暴露出来。
const QUIET_PERIOD: Duration = Duration::from_secs(3);

/// 记录型关机动作的失败详情：**只允许出现在诊断输出里**，因此用例断言响应体里绝不出现它。
const FAILURE_DETAIL: &str = "recording-power-off-detail-canary-7f3a";

/// 记录型关机动作：记录调用次数，并按构造时指定的结果返回——**绝不**接触系统关机。
///
/// 它能分别返回成功与失败，因此 `200` 与 `500 shutdown-failed` 两条语义都能被驱动。
struct RecordingPowerOff {
    /// 本次记录型动作要返回的结果。
    outcome: RecordingOutcome,
    /// 被调用的次数。
    calls: AtomicUsize,
}

/// 记录型动作的结果：成功（关机已发出）或失败（附一段只该进诊断输出的详情）。
enum RecordingOutcome {
    Issued,
    Fails(&'static str),
}

impl RecordingPowerOff {
    /// 成功返回的替身：断言“空 POST → 200 且动作被调用恰好一次”。
    fn succeeding() -> Arc<Self> {
        Arc::new(Self {
            outcome: RecordingOutcome::Issued,
            calls: AtomicUsize::new(0),
        })
    }

    /// 失败返回的替身：断言“动作错误 → 500 `shutdown-failed`”。
    fn failing() -> Arc<Self> {
        Arc::new(Self {
            outcome: RecordingOutcome::Fails(FAILURE_DETAIL),
            calls: AtomicUsize::new(0),
        })
    }

    /// 到目前为止被调用了几次。
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl omp_bot::PowerOff for RecordingPowerOff {
    fn power_off(&self) -> Result<(), omp_bot::PowerOffFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.outcome {
            RecordingOutcome::Issued => Ok(()),
            RecordingOutcome::Fails(detail) => {
                Err(omp_bot::PowerOffFailure::Privilege(detail.to_string()))
            }
        }
    }
}

/// 关机用例的服务端配置：两个固定操作绑定**不同**夹具，因此调用记录能区分“哪个脚本被调用过”。
///
/// “启动”绑定长跑夹具（靠它占住执行资格），“停止”绑定立即失败的夹具（任何“停止”调用都会留下记录）。
fn shutdown_config(tree: &FixtureTree) -> omp_bot::Config {
    let mut config = config(&tree.script("long-running.bat"), &tree.operations());
    config.operations.stop.script = tree.script("failure.bat");
    config
}

/// 装配注入了记录型关机动作的应用：路由与交付运行时完全一致，只是动作换成了替身。
fn shutdown_app(tree: &FixtureTree, recording: &Arc<RecordingPowerOff>) -> axum::Router {
    let power_off: Arc<dyn omp_bot::PowerOff> = recording.clone();
    omp_bot::app_with_power_off(&shutdown_config(tree), power_off)
}

/// 发一次请求，返回状态码与原始响应体文本。
async fn send_raw(app: axum::Router, request: Request<Body>) -> (StatusCode, String) {
    let response = app.oneshot(request).await.expect("请求应用");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("读取响应体");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// 发一次请求，并要求响应体是 JSON。
async fn send(app: axum::Router, request: Request<Body>) -> (StatusCode, String, Value) {
    let (status, raw) = send_raw(app, request).await;
    let json = serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("响应体必须是 JSON（{error}）：{raw}"));
    (status, raw, json)
}

/// 裸 `POST <path>`：没有查询串，也没有请求体。
fn bare_post(uri: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .expect("构造请求")
}

/// 裸 `POST /api/shutdown`。
async fn post_bare_shutdown(app: axum::Router) -> (StatusCode, String, Value) {
    send(app, bare_post(SHUTDOWN)).await
}

/// 响应体的 JSON。
fn body(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body)
        .unwrap_or_else(|error| panic!("响应体必须是 JSON（{error}）：{}", response.body))
}

/// 某个夹具的真实调用记录（`stage = invoke` 的行）。
fn invokes(tree: &FixtureTree, fixture: &str) -> Vec<common::CallRecord> {
    tree.records()
        .into_iter()
        .filter(|record| record.fixture == fixture && record.stage == "invoke")
        .collect()
}

/// 提交一次“启动”请求并等到长跑夹具真的开始执行（**正对照**：单次请求确实产生一次调用记录），
/// 返回仍在等待响应的请求与正在运行的夹具 PID。
async fn hold_start(
    server: &TestServer,
    tree: &FixtureTree,
) -> (JoinHandle<HttpResponse>, u32) {
    let addr = server.addr();
    let request = tokio::spawn(async move { post_over_tcp(addr, START).await });

    let recorded =
        common::wait_until(Duration::from_secs(30), || !invokes(tree, "long-running").is_empty())
            .await;
    assert!(
        recorded,
        "正对照：单次“启动”请求必须真实产生一次长跑夹具调用：{:#?}",
        tree.records()
    );

    let pid = invokes(tree, "long-running").remove(0).pid;
    assert!(
        common::wait_until(Duration::from_secs(15), || common::process_is_alive(pid)).await,
        "长跑夹具必须处于运行中（PID {pid}）"
    );
    (request, pid)
}

/// 经人工结束入口结束长跑夹具，并等回正在执行的“启动”请求。
async fn end_start(tree: &FixtureTree, request: JoinHandle<HttpResponse>) -> HttpResponse {
    let stop_output = tree.stop_fixtures();
    assert!(
        stop_output.status.success(),
        "结束脚本必须成功退出：{stop_output:?}"
    );

    let response = tokio::time::timeout(Duration::from_secs(30), request)
        .await
        .expect("脚本结束后正在执行的请求必须返回")
        .expect("正在执行的请求任务");
    assert!(
        common::wait_until(Duration::from_secs(15), || invokes(tree, "long-running")
            .iter()
            .all(|invoke| !common::process_is_alive(invoke.pid)))
        .await,
        "验证后不得遗留运行中的夹具进程"
    );
    response
}

/// 验收 1：空 `POST /api/shutdown` → `200`，记录型关机动作被调用**恰好一次**，且不执行任何脚本。
#[tokio::test]
async fn empty_shutdown_post_returns_200_and_invokes_the_action_exactly_once() {
    let tree = FixtureTree::new("shutdown-invoked");
    let recording = RecordingPowerOff::succeeding();

    let (status, raw, body) = post_bare_shutdown(shutdown_app(&tree, &recording)).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "无参数调用必须成功返回 200（关机已发出）：{raw}"
    );
    assert_eq!(
        body["outcome"], "shutdown-issued",
        "成功结果必须给出可断言的类别：{body}"
    );
    assert_eq!(
        recording.calls(),
        1,
        "一次请求必须触发恰好一次关机动作：{body}"
    );
    assert!(
        tree.records().is_empty(),
        "关机路径不执行任何脚本（既不是“启动”也不是“停止”）：{:#?}",
        tree.records()
    );
}

/// 验收 2：带查询串或请求体的 `POST /api/shutdown` → `400 unexpected-parameters`，动作**未被调用**。
#[tokio::test]
async fn parameter_carrying_shutdown_requests_are_rejected_without_any_action() {
    let tree = FixtureTree::new("shutdown-rejected");
    let recording = RecordingPowerOff::succeeding();

    let with_query = Request::builder()
        .method("POST")
        .uri("/api/shutdown?delay=0")
        .body(Body::empty())
        .expect("构造带查询串的请求");
    let with_body = Request::builder()
        .method("POST")
        .uri(SHUTDOWN)
        .body(Body::from(r#"{"force":true}"#))
        .expect("构造带请求体的请求");

    for (label, request) in [("查询串", with_query), ("请求体", with_body)] {
        let (status, raw, body) = send(shutdown_app(&tree, &recording), request).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{label}必须被拒绝为 400：{raw}"
        );
        assert_eq!(
            body["code"], "unexpected-parameters",
            "{label}必须按既有类别拒绝（本服务不接受任何参数）：{body}"
        );
    }

    assert_eq!(
        recording.calls(),
        0,
        "被拒绝的请求不得触发任何关机动作（无副作用）"
    );
    assert!(
        tree.records().is_empty(),
        "被拒绝的请求不执行任何脚本：{:#?}",
        tree.records()
    );
}

/// 验收 3：动作返回错误 → `500 shutdown-failed`；响应体只给类别，错误详情只进诊断输出。
#[tokio::test]
async fn failing_shutdown_action_returns_500_shutdown_failed_without_leaking_the_detail() {
    let tree = FixtureTree::new("shutdown-failed");
    let recording = RecordingPowerOff::failing();

    let (status, raw, body) = post_bare_shutdown(shutdown_app(&tree, &recording)).await;

    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "动作失败必须如实回 500，不得当作成功：{raw}"
    );
    assert_eq!(
        body["code"], "shutdown-failed",
        "失败必须给出可断言的类别：{body}"
    );
    assert!(
        !raw.contains(FAILURE_DETAIL),
        "错误详情只允许出现在诊断输出里，不得进入响应体：{raw}"
    );
    assert_eq!(
        recording.calls(),
        1,
        "失败的一次请求同样只触发一次关机动作：{body}"
    );
}

/// 验收 4：长跑夹具占着执行资格时，`POST /api/shutdown` 仍被接受并成功；同期到达的固定操作请求
/// 仍得到既有忙碌语义（状态码与类别不变），且脚本与资格都不受关机影响。
#[tokio::test(flavor = "multi_thread")]
async fn shutdown_is_accepted_while_a_script_operation_holds_the_execution_permit() {
    let tree = FixtureTree::new("shutdown-busy");
    let recording = RecordingPowerOff::succeeding();
    let server = TestServer::start_app(shutdown_app(&tree, &recording)).await;

    let (start_request, pid) = hold_start(&server, &tree).await;

    // 脚本还在执行：关机照样被接受并成功——它不获取执行资格。
    let shutdown = post_over_tcp(server.addr(), SHUTDOWN).await;
    assert_eq!(
        shutdown.status, 200,
        "脚本执行期间的关机请求必须被接受并成功：{shutdown:?}"
    );
    assert_eq!(
        recording.calls(),
        1,
        "关机动作必须被调用恰好一次：{shutdown:?}"
    );

    // 忙碌语义不变：同期到达的固定操作请求仍得到既有类别。
    let stop = post_over_tcp(server.addr(), STOP).await;
    assert_eq!(
        stop.status, BUSY_STATUS,
        "脚本执行期间的“停止”请求必须仍是忙碌结果：{stop:?}"
    );
    assert_eq!(
        body(&stop)["code"],
        BUSY_CODE,
        "忙碌类别不得改变：{}",
        stop.body
    );

    // 关机既没有结束正在执行的脚本，也没有释放资格。
    assert!(
        common::process_is_alive(pid),
        "关机不得结束正在执行的脚本（PID {pid}）"
    );
    assert!(
        !start_request.is_finished(),
        "关机不得释放执行资格：正在执行的请求不得提前返回"
    );

    let completed = end_start(&tree, start_request).await;
    assert_eq!(
        completed.status, 200,
        "资格释放后正在执行的请求必须正常返回执行结果：{completed:?}"
    );
    assert_eq!(
        body(&completed)["outcome"],
        "completed",
        "长跑夹具被停止后正常结束：{}",
        completed.body
    );
}

/// 验收 5：关机路径不执行“停止”脚本，也不获取或释放执行资格（脚本继续跑、既不产生额外调用，
/// 也不把请求排队补做），资格在关机之后照常可用。
#[tokio::test(flavor = "multi_thread")]
async fn shutdown_runs_no_stop_script_and_leaves_the_execution_permit_untouched() {
    let tree = FixtureTree::new("shutdown-permit");
    let recording = RecordingPowerOff::succeeding();
    let server = TestServer::start_app(shutdown_app(&tree, &recording)).await;

    let (start_request, pid) = hold_start(&server, &tree).await;

    let shutdown = post_over_tcp(server.addr(), SHUTDOWN).await;
    assert_eq!(shutdown.status, 200, "关机必须成功：{shutdown:?}");

    // 关机不执行“停止”脚本：两个操作绑定不同夹具，任何停止调用都会在记录里留下 failure 的行。
    assert!(
        invokes(&tree, "failure").is_empty(),
        "关机不得执行“停止”脚本：{:#?}",
        tree.records()
    );
    assert!(
        common::process_is_alive(pid),
        "关机不得结束正在执行的脚本（PID {pid}）"
    );
    assert!(
        !start_request.is_finished(),
        "关机不得获取或释放执行资格：正在执行的请求必须照旧等待脚本结束"
    );

    let completed = end_start(&tree, start_request).await;
    assert_eq!(completed.status, 200, "正在执行的请求必须正常返回：{completed:?}");

    // 静置：关机没有把任何请求排队，也没有延迟补做。
    tokio::time::sleep(QUIET_PERIOD).await;
    assert_eq!(
        invokes(&tree, "long-running").len(),
        1,
        "关机不得产生额外调用或延迟补做（静置 {QUIET_PERIOD:?}）：{:#?}",
        tree.records()
    );
    assert!(
        invokes(&tree, "failure").is_empty(),
        "关机始终不得触发“停止”脚本：{:#?}",
        tree.records()
    );

    // 资格没有被关机弄坏：随后提交的固定操作照常被接受并真实执行。
    let stop = post_over_tcp(server.addr(), STOP).await;
    assert_eq!(
        stop.status, 200,
        "关机之后固定操作必须照常可用（资格照常获取与释放）：{stop:?}"
    );
    assert_eq!(
        body(&stop)["outcome"],
        "failed",
        "failure.bat 以退出码 3 结束（脚本确实被执行）：{}",
        stop.body
    );
    assert_eq!(invokes(&tree, "failure").len(), 1);
}

/// 验收 6：端点永远注册（不是能力接口、不由配置或前端决定是否可用）；`/api/` 下其他未知路径仍按
/// 既有类别拒绝。
///
/// 探测**生产装配**（真实的 Win32 动作）时只用 GET：`GET /api/shutdown` 永远不触发关机动作，
/// 因此本用例不可能真的关机（POST 的语义由记录型装配覆盖）。
#[tokio::test]
async fn shutdown_route_is_always_registered_and_other_api_paths_keep_their_categories() {
    let tree = FixtureTree::new("shutdown-registration");

    for mdns_enabled in [true, false] {
        let mut config = shutdown_config(&tree);
        config.mdns = omp_bot::MdnsConfig {
            enabled: mdns_enabled,
            address: None,
        };
        let get = Request::builder()
            .method("GET")
            .uri(SHUTDOWN)
            .body(Body::empty())
            .expect("构造 GET 请求");

        let (status, raw) = send_raw(omp_bot::app(&config), get).await;
        assert_eq!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "交付装配必须注册关机端点（换个 HTTP 方法得到 405，而不是 404）：{raw}"
        );
    }

    let recording = RecordingPowerOff::succeeding();
    for (path, code) in [
        ("/api/unknown", "unknown-endpoint"),
        ("/api/operations/format", "unknown-operation"),
        ("/api/shutdown/extra", "unknown-endpoint"),
    ] {
        let (status, raw, body) = send(shutdown_app(&tree, &recording), bare_post(path)).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{path} 必须仍按既有类别拒绝：{raw}"
        );
        assert_eq!(body["code"], code, "{path} 的类别不得改变：{body}");
    }

    assert_eq!(recording.calls(), 0, "注册探测不得触发任何关机动作");
    assert!(
        tree.records().is_empty(),
        "注册探测不得执行任何脚本：{:#?}",
        tree.records()
    );
}

/// 验收 7：诊断输出记录关机动作 `invoked` / `failed` 事实，类别机器可断言。
///
/// 诊断行写在进程输出里（S3 诊断日志 seam），而进程内拿不到自己的输出，因此这里让**同一个测试
/// 二进制**以 `--exact --nocapture` 在子进程里跑一次真实的端点路径（记录型动作，绝不真的关机），
/// 由父进程断言子进程的原始输出里两个类别行都在、且互相区分。
#[test]
fn diagnostics_of_the_exercised_shutdown_path_are_machine_assertable() {
    let exe = std::env::current_exe().expect("测试二进制路径");
    let output = std::process::Command::new(exe)
        .args([
            "--exact",
            "--nocapture",
            "shutdown_diagnostics_on_the_exercised_path",
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
        log.contains("[omp-bot] 关机状态：invoked"),
        "动作成功必须留下 invoked 类别：\n{log}"
    );
    assert!(
        log.contains("[omp-bot] 关机状态：failed step=privilege error=") && log.contains(FAILURE_DETAIL),
        "动作失败必须留下 failed 类别与系统原始错误（详情只出现在这里）：\n{log}"
    );
}

/// 子进程用例：在真实端点路径上各发一次失败与成功的关机请求（都用记录型动作）。
#[tokio::test]
async fn shutdown_diagnostics_on_the_exercised_path() {
    let tree = FixtureTree::new("shutdown-diagnostics");

    let failing = RecordingPowerOff::failing();
    let (status, _, _) = post_bare_shutdown(shutdown_app(&tree, &failing)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let succeeding = RecordingPowerOff::succeeding();
    let (status, _, _) = post_bare_shutdown(shutdown_app(&tree, &succeeding)).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(tree.records().len(), 0, "关机不执行任何脚本");
}

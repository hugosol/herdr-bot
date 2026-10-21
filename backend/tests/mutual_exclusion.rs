//! 单实例互斥与忙碌拒绝（契约 P8）：同一后端实例内任意时刻**至多一个**脚本操作在执行，
//! 重叠请求立即得到忙碌结果，并且在当前操作结束后**不会**被补做。
//!
//! 全部经 S2 HTTP interface 驱动，而且请求都由测试自己经**独立 TCP 连接**发出（`post_over_tcp`），
//! 完全不经过页面与按钮状态：某个操作正在执行时，绕过页面直接提交同样只会得到忙碌结果。
//!
//! 判定依据来自夹具自己写下的调用记录（`fixtures/README.md`）：某个夹具的 `invoke` 行数就是它被真实
//! 调用的次数。长跑夹具 `long-running.bat` 只有在人工结束入口写入停止标记后才结束，因此重叠窗口
//! 完全由测试控制：脚本没有结束之前，任何“等待前一次操作结束再执行”的实现都会超时失败。

mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use common::{CallRecord, FixtureTree, HttpResponse, TestServer, config, post_over_tcp};
use tokio::task::JoinHandle;

/// 忙碌结果的编码（见根 `README.md` 的“操作进行中”一节）。
const BUSY_STATUS: u16 = 409;
const BUSY_CODE: &str = "operation-in-progress";

/// “启动”操作的请求路径。
const START: &str = "/api/operations/start";
/// “停止”操作的请求路径。
const STOP: &str = "/api/operations/stop";

/// 重叠请求的上限耗时。正在执行的脚本会一直不结束，因此一旦有请求排队等待它，
/// 这个上限就会让它以可读的方式失败。
const BUSY_DEADLINE: Duration = Duration::from_secs(5);

/// 忙碌响应必须立即返回：脚本仍在执行时，重叠请求的耗时应当是这个上限的零头。
const BUSY_BUDGET: Duration = Duration::from_secs(2);

/// 操作结束后的静置时间：足够让“排队补做”暴露出来。
const QUIET_PERIOD: Duration = Duration::from_secs(3);

/// 把两个固定操作分别绑定到不同夹具，便于用调用记录区分“哪个操作真的执行了”。
fn config_with(tree: &FixtureTree, start: &str, stop: &str) -> herdr_bot::Config {
    let mut config = config(&tree.script(start), &tree.operations());
    config.operations.stop.script = tree.script(stop);
    config
}

/// 某个夹具的真实调用记录（`stage = invoke` 的行）。
fn invokes(tree: &FixtureTree, fixture: &str) -> Vec<CallRecord> {
    tree.records()
        .into_iter()
        .filter(|record| record.fixture == fixture && record.stage == "invoke")
        .collect()
}

/// 提交一次操作请求，并等到夹具真的开始执行：**正对照**，证明单次请求确实产生一次调用记录。
///
/// 返回仍在等待响应的请求任务与正在运行的夹具进程 PID。
async fn hold(
    server: &TestServer,
    tree: &FixtureTree,
    path: &'static str,
    fixture: &str,
) -> (JoinHandle<HttpResponse>, u32) {
    let addr = server.addr();
    let request = tokio::spawn(async move { post_over_tcp(addr, path).await });

    let recorded = common::wait_until(Duration::from_secs(30), || !invokes(tree, fixture).is_empty())
        .await;
    assert!(
        recorded,
        "正对照：单次请求必须真实产生一次 {fixture} 调用记录：{:#?}",
        tree.records()
    );

    let invoke = invokes(tree, fixture).remove(0);
    assert!(
        common::process_is_alive(invoke.pid),
        "{fixture} 必须处于运行中（PID {}）",
        invoke.pid
    );
    assert!(
        !request.is_finished(),
        "脚本未结束时，正在执行的请求不得提前返回"
    );
    (request, invoke.pid)
}

/// 由多个独立客户端同时提交若干操作请求，返回每个请求的路径与响应。
///
/// 每个请求都是一条新的 TCP 连接（独立 HTTP 客户端），并各自有超时上限：只要有请求在等待
/// 前一次操作结束，它就会在这里失败，而不是把整个测试挂住。
async fn challenge_all(
    addr: SocketAddr,
    requests: &[&'static str],
) -> Vec<(&'static str, HttpResponse)> {
    let mut tasks = Vec::new();
    for path in requests {
        let path: &'static str = path;
        tasks.push((
            path,
            tokio::spawn(async move {
                tokio::time::timeout(BUSY_DEADLINE, post_over_tcp(addr, path))
                    .await
                    .ok()
            }),
        ));
    }

    let mut responses = Vec::new();
    for (path, task) in tasks {
        let response = task
            .await
            .expect("并发客户端任务")
            .unwrap_or_else(|| panic!("重叠请求必须立即返回忙碌结果，不得排队等待前一次操作结束：{path}"));
        responses.push((path, response));
    }
    responses
}

/// 断言响应是忙碌结果，而不是执行结果或形状拒绝。
fn assert_busy(path: &str, response: &HttpResponse) {
    assert_eq!(
        response.status, BUSY_STATUS,
        "{path} 在操作执行期间必须得到忙碌结果：{response:?}"
    );
    let body: serde_json::Value =
        serde_json::from_str(&response.body).expect("忙碌响应必须是 JSON");
    assert_eq!(body["code"], BUSY_CODE, "{path} 的忙碌类别必须可区分：{body}");
    assert!(
        body.get("outcome").is_none(),
        "{path} 的忙碌响应不得伪装成一次执行结果：{body}"
    );
}

/// 经人工结束入口结束长跑夹具，等回正在执行的请求，并确认没有遗留进程。
async fn end_running_operation(tree: &FixtureTree, request: JoinHandle<HttpResponse>, pid: u32) {
    let stop_output = tree.stop_fixtures();
    assert!(
        stop_output.status.success(),
        "结束脚本必须成功退出：{stop_output:?}"
    );

    let response = tokio::time::timeout(Duration::from_secs(30), request)
        .await
        .expect("脚本结束后正在执行的请求必须返回")
        .expect("正在执行的请求任务");
    assert_eq!(
        response.status, 200,
        "正在执行的操作必须正常返回执行结果：{response:?}"
    );
    let body: serde_json::Value =
        serde_json::from_str(&response.body).expect("操作响应必须是 JSON");
    assert_eq!(body["outcome"], "completed", "长跑夹具被停止后正常结束：{body}");

    assert!(
        common::wait_until(Duration::from_secs(15), || !common::process_is_alive(pid)).await,
        "验证后不得遗留运行中的夹具进程（PID {pid}）"
    );
}

/// 操作结束后再静置一段时间：先前被拒绝的请求不得被补做。
async fn assert_nothing_ran_later(tree: &FixtureTree) {
    tokio::time::sleep(QUIET_PERIOD).await;
    let records = tree.records();
    assert_eq!(
        records
            .iter()
            .filter(|record| record.stage == "invoke")
            .count(),
        1,
        "操作结束后不得出现被延迟补做的调用（静置 {QUIET_PERIOD:?}）：{records:#?}"
    );
}

/// 启动/启动：一个“启动”正在执行时，其他客户端提交的“启动”立即得到忙碌结果。
#[tokio::test(flavor = "multi_thread")]
async fn overlapping_start_requests_run_the_script_once_and_the_rest_are_busy() {
    let tree = FixtureTree::new("busy-start-start");
    let server = TestServer::start(&config_with(&tree, "long-running.bat", "failure.bat")).await;

    let (request, pid) = hold(&server, &tree, START, "long-running").await;

    let started = Instant::now();
    let responses = challenge_all(server.addr(), &[START, START, START]).await;
    let elapsed = started.elapsed();

    for (path, response) in &responses {
        assert_busy(path, response);
    }
    assert!(
        elapsed < BUSY_BUDGET,
        "忙碌响应必须立即返回，不能等到前一次操作结束（耗时 {elapsed:?}）：{responses:#?}"
    );
    assert!(
        !request.is_finished(),
        "忙碌响应返回时，前一次操作必须仍在执行"
    );
    assert!(
        common::process_is_alive(pid),
        "忙碌响应返回时，长跑夹具必须仍在运行（PID {pid}）"
    );

    assert_eq!(
        invokes(&tree, "long-running").len(),
        1,
        "整段重叠期间“启动”只能真实执行一次：{:#?}",
        tree.records()
    );
    assert_eq!(
        invokes(&tree, "failure").len(),
        0,
        "重叠期间“停止”不得被执行：{:#?}",
        tree.records()
    );

    end_running_operation(&tree, request, pid).await;
    assert_nothing_ran_later(&tree).await;

    // 执行资格在操作结束后释放：此时新的合法请求不再被拒绝为忙碌，而是真的执行。
    let fresh = post_over_tcp(server.addr(), STOP).await;
    assert_eq!(
        fresh.status, 200,
        "操作结束后新的合法请求必须被接受：{fresh:?}"
    );
    let body: serde_json::Value =
        serde_json::from_str(&fresh.body).expect("操作响应必须是 JSON");
    assert_eq!(body["started"], true, "新的合法请求必须真的执行脚本：{body}");
    assert_eq!(
        invokes(&tree, "failure").len(),
        1,
        "操作结束后的新请求必须产生一次调用：{:#?}",
        tree.records()
    );
}

/// 停止/停止：一个“停止”正在执行时，其他客户端提交的“停止”立即得到忙碌结果。
#[tokio::test(flavor = "multi_thread")]
async fn overlapping_stop_requests_run_the_script_once_and_the_rest_are_busy() {
    let tree = FixtureTree::new("busy-stop-stop");
    let server = TestServer::start(&config_with(&tree, "failure.bat", "long-running.bat")).await;

    let (request, pid) = hold(&server, &tree, STOP, "long-running").await;

    let started = Instant::now();
    let responses = challenge_all(server.addr(), &[STOP, STOP, STOP]).await;
    let elapsed = started.elapsed();

    for (path, response) in &responses {
        assert_busy(path, response);
    }
    assert!(
        elapsed < BUSY_BUDGET,
        "忙碌响应必须立即返回，不能等到前一次操作结束（耗时 {elapsed:?}）：{responses:#?}"
    );
    assert!(
        !request.is_finished(),
        "忙碌响应返回时，前一次操作必须仍在执行"
    );
    assert!(
        common::process_is_alive(pid),
        "忙碌响应返回时，长跑夹具必须仍在运行（PID {pid}）"
    );

    assert_eq!(
        invokes(&tree, "long-running").len(),
        1,
        "整段重叠期间“停止”只能真实执行一次：{:#?}",
        tree.records()
    );
    assert_eq!(
        invokes(&tree, "failure").len(),
        0,
        "重叠期间“启动”不得被执行：{:#?}",
        tree.records()
    );

    end_running_operation(&tree, request, pid).await;
    assert_nothing_ran_later(&tree).await;
}

/// 启动/停止：“启动”正在执行时，其他客户端提交的“停止”立即得到忙碌结果。
#[tokio::test(flavor = "multi_thread")]
async fn stop_requests_are_busy_while_a_start_is_running() {
    let tree = FixtureTree::new("busy-start-stop");
    let server = TestServer::start(&config_with(&tree, "long-running.bat", "failure.bat")).await;

    let (request, pid) = hold(&server, &tree, START, "long-running").await;

    let started = Instant::now();
    let responses = challenge_all(server.addr(), &[STOP, STOP, STOP]).await;
    let elapsed = started.elapsed();

    for (path, response) in &responses {
        assert_busy(path, response);
    }
    assert!(
        elapsed < BUSY_BUDGET,
        "忙碌响应必须立即返回，不能等到前一次操作结束（耗时 {elapsed:?}）：{responses:#?}"
    );
    assert!(
        !request.is_finished(),
        "忙碌响应返回时，前一次操作必须仍在执行"
    );

    assert_eq!(
        invokes(&tree, "long-running").len(),
        1,
        "整段重叠期间“启动”只能真实执行一次：{:#?}",
        tree.records()
    );
    assert_eq!(
        invokes(&tree, "failure").len(),
        0,
        "被拒绝的“停止”不得留下任何调用记录：{:#?}",
        tree.records()
    );

    end_running_operation(&tree, request, pid).await;
    assert_nothing_ran_later(&tree).await;
    assert_eq!(
        invokes(&tree, "failure").len(),
        0,
        "操作结束后被拒绝的“停止”也不得被补做：{:#?}",
        tree.records()
    );
}

/// 停止/启动：“停止”正在执行时，其他客户端提交的“启动”立即得到忙碌结果。
#[tokio::test(flavor = "multi_thread")]
async fn start_requests_are_busy_while_a_stop_is_running() {
    let tree = FixtureTree::new("busy-stop-start");
    let server = TestServer::start(&config_with(&tree, "failure.bat", "long-running.bat")).await;

    let (request, pid) = hold(&server, &tree, STOP, "long-running").await;

    let started = Instant::now();
    let responses = challenge_all(server.addr(), &[START, START, START]).await;
    let elapsed = started.elapsed();

    for (path, response) in &responses {
        assert_busy(path, response);
    }
    assert!(
        elapsed < BUSY_BUDGET,
        "忙碌响应必须立即返回，不能等到前一次操作结束（耗时 {elapsed:?}）：{responses:#?}"
    );
    assert!(
        !request.is_finished(),
        "忙碌响应返回时，前一次操作必须仍在执行"
    );

    assert_eq!(
        invokes(&tree, "long-running").len(),
        1,
        "整段重叠期间“停止”只能真实执行一次：{:#?}",
        tree.records()
    );
    assert_eq!(
        invokes(&tree, "failure").len(),
        0,
        "被拒绝的“启动”不得留下任何调用记录：{:#?}",
        tree.records()
    );

    end_running_operation(&tree, request, pid).await;
    assert_nothing_ran_later(&tree).await;
    assert_eq!(
        invokes(&tree, "failure").len(),
        0,
        "操作结束后被拒绝的“启动”也不得被补做：{:#?}",
        tree.records()
    );
}

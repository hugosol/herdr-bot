//! 连接中断与操作生命周期（契约 P10）：已接受的操作不因请求连接中断而取消，执行资格也不因断线
//! 提前释放——断线后脚本继续执行到它自己结束，期间其他客户端仍然得到忙碌结果，脚本确认结束后恢复
//! 接受操作；断线不触发取消、结果恢复或补做。
//!
//! 断线是**真实**的客户端中止：测试自己打开一条 TCP 连接发出裸 POST，在脚本仍在运行时关闭这条
//! socket（不读响应、不发任何取消请求，也不经过页面与按钮状态）。观测点全部来自夹具自己写下的调用
//! 记录与进程存活：预期“一次完整调用”“期间无第二次调用”“结束后恢复接受并真的执行”都由夹具的
//! 真实记录给出，不由实现自身计算得出。

mod common;

use std::time::{Duration, Instant};

use common::{CallRecord, FixtureTree, HttpResponse, OpenRequest, TestServer, config, post_over_tcp};

/// 忙碌结果的编码（见根 `README.md` 的“操作进行中”一节）。
const BUSY_STATUS: u16 = 409;
const BUSY_CODE: &str = "operation-in-progress";

/// “启动”与“停止”操作的请求路径。
const START: &str = "/api/operations/start";
const STOP: &str = "/api/operations/stop";

/// 断线后、提交其他请求之前的宽限时间：足够服务端读取到连接结束。
const DISCONNECT_SETTLE: Duration = Duration::from_secs(2);

/// 忙碌响应的耗时上限：连接结束不得连带放掉执行资格，
/// 否则重叠请求会变成第二次真实调用，而不是立即返回忙碌。
const BUSY_BUDGET: Duration = Duration::from_secs(2);

/// 单次探测请求的耗时上限：超过它说明请求被执行中的脚本挡住（等待脚本结束），
/// 而不是立即得到忙碌结果。
const BUSY_DEADLINE: Duration = Duration::from_secs(5);

/// 操作结束后的静置时间：足够让“断线造成的重放或补做”暴露出来。
const QUIET_PERIOD: Duration = Duration::from_secs(3);

/// 等待夹具记录/进程状态变化的上限。
const RECORD_DEADLINE: Duration = Duration::from_secs(30);

/// 提交一次操作请求并限定耗时：请求挂住说明它被执行中的脚本挡住了，
/// 而本票要求的观察点是“立即返回的忙碌结果”，因此挂住本身就是失败。
async fn probe(addr: std::net::SocketAddr, path: &str, deadline: Duration) -> HttpResponse {
    match tokio::time::timeout(deadline, post_over_tcp(addr, path)).await {
        Ok(response) => response,
        Err(_) => panic!(
            "{path} 必须在 {deadline:?} 内返回：挂住说明这次请求被当成了正在执行的操作并等待脚本结束"
        ),
    }
}

/// 夹具调用记录里某个夹具的 `invoke` 行数就是它被真实调用的次数。
fn invokes(tree: &FixtureTree, fixture: &str) -> Vec<CallRecord> {
    tree.records()
        .into_iter()
        .filter(|record| record.fixture == fixture && record.stage == "invoke")
        .collect()
}

/// 某个夹具的全部记录（各阶段）。
fn records_of(tree: &FixtureTree, fixture: &str) -> Vec<CallRecord> {
    tree.records()
        .into_iter()
        .filter(|record| record.fixture == fixture)
        .collect()
}

/// 断言响应是忙碌结果，而不是执行结果或形状拒绝。
fn assert_busy(path: &str, response: &HttpResponse) {
    assert_eq!(
        response.status, BUSY_STATUS,
        "{path} 在断线后的操作执行期间必须得到忙碌结果：{response:?}"
    );
    let body: serde_json::Value =
        serde_json::from_str(&response.body).expect("忙碌响应必须是 JSON");
    assert_eq!(body["code"], BUSY_CODE, "{path} 的忙碌类别必须可区分：{body}");
    assert!(
        body.get("outcome").is_none(),
        "{path} 的忙碌响应不得伪装成一次执行结果：{body}"
    );
}

/// “启动”绑定长跑夹具（只有停止标记能让它结束），“停止”绑定立即成功结束的夹具：
/// 于是“结束后恢复接受操作”能用另一个夹具的真实调用记录证明。
fn config_for(tree: &FixtureTree) -> herdr_bot::Config {
    let mut config = config(&tree.script("long-running.bat"), &tree.operations());
    config.operations.stop.script = tree.script("success.bat");
    config
}

/// 连接中断不取消已接受的操作，也不提前释放执行资格。
#[tokio::test(flavor = "multi_thread")]
async fn dropped_connection_keeps_the_operation_running_until_its_own_end() {
    let tree = FixtureTree::new("disconnect-lifecycle");
    let server = TestServer::start(&config_for(&tree)).await;

    // 客户端提交“启动”后不读响应，把连接留在手里：脚本仍在运行时这条连接会被主动关闭。
    let client = OpenRequest::submit(server.addr(), START).await;

    // 正对照：这次请求确实已被接受，并产生一次真实调用，夹具正在运行。
    let accepted =
        common::wait_until(RECORD_DEADLINE, || !invokes(&tree, "long-running").is_empty()).await;
    assert!(
        accepted,
        "正对照：提交后必须产生一次 long-running 调用记录：{:#?}",
        tree.records()
    );
    let invoked = invokes(&tree, "long-running").remove(0);
    assert!(
        common::process_is_alive(invoked.pid),
        "断线前夹具必须正在运行（PID {}）",
        invoked.pid
    );

    // 真实断线：客户端关闭这条 TCP 连接，既不读响应，也不发任何取消请求。
    client.abort();
    tokio::time::sleep(DISCONNECT_SETTLE).await;

    // 断线不取消已接受的操作：脚本仍在运行，也没有被调用第二次。
    assert!(
        common::process_is_alive(invoked.pid),
        "断线不得取消已接受的操作：夹具必须继续运行（PID {}）",
        invoked.pid
    );
    assert_eq!(
        invokes(&tree, "long-running").len(),
        1,
        "断线不得产生第二次调用：{:#?}",
        tree.records()
    );

    // 断线期间其他客户端提交：两个固定操作都立即得到忙碌结果，而不是开始第二次调用。
    let started = Instant::now();
    let busy_start = probe(server.addr(), START, BUSY_DEADLINE).await;
    let busy_stop = probe(server.addr(), STOP, BUSY_DEADLINE).await;
    let elapsed = started.elapsed();
    assert_busy(START, &busy_start);
    assert_busy(STOP, &busy_stop);
    assert!(
        elapsed < BUSY_BUDGET,
        "断线期间的重叠请求必须立即返回忙碌结果（耗时 {elapsed:?}）"
    );
    assert_eq!(
        invokes(&tree, "long-running").len(),
        1,
        "忙碌请求不得开始第二次调用：{:#?}",
        tree.records()
    );
    assert_eq!(
        invokes(&tree, "success").len(),
        0,
        "忙碌请求不得执行任何脚本：{:#?}",
        tree.records()
    );
    assert!(
        common::process_is_alive(invoked.pid),
        "忙碌响应返回时，断线那次调用的夹具必须仍在运行（PID {}）",
        invoked.pid
    );

    // 断线那次的脚本仍能被按它自己的方式结束：停止标记 → 夹具自己写结束记录并以 0 退出。
    let stop_output = tree.stop_fixtures();
    assert!(
        stop_output.status.success(),
        "结束脚本必须成功退出：{stop_output:?}"
    );
    assert!(
        common::wait_until(Duration::from_secs(15), || !common::process_is_alive(
            invoked.pid
        ))
        .await,
        "断线后的操作必须执行到它自己的结束，不得遗留进程（PID {}）",
        invoked.pid
    );

    let long_running = records_of(&tree, "long-running");
    assert_eq!(
        long_running.len(),
        2,
        "一次调用必须留下开始与结束两条记录：{long_running:#?}"
    );
    assert_eq!(long_running[0].stage, "invoke");
    assert_eq!(
        long_running[1].stage, "stop-observed",
        "夹具必须自己看到停止标记后才结束：{long_running:#?}"
    );
    assert_eq!(
        long_running[0].call_id, long_running[1].call_id,
        "开始与结束必须属于同一次调用：{long_running:#?}"
    );
    assert_eq!(long_running[0].call, long_running[1].call);
    let stop_record = records_of(&tree, "stop-fixtures");
    assert_eq!(stop_record.len(), 1, "结束脚本必须留下一条记录");
    assert!(
        stop_record[0]
            .detail
            .contains("long-running=ended-by-stop-marker"),
        "夹具必须由停止标记按自己的方式结束：{}",
        stop_record[0].detail
    );

    // 脚本确认结束后恢复接受操作：下一次“停止”被接受并真的执行（成功夹具以 0 结束）。
    let recovered = probe(server.addr(), STOP, RECORD_DEADLINE).await;
    assert_eq!(
        recovered.status, 200,
        "脚本结束后必须恢复接受操作：{recovered:?}"
    );
    let body: serde_json::Value =
        serde_json::from_str(&recovered.body).expect("操作响应必须是 JSON");
    assert_eq!(
        body["outcome"], "completed",
        "恢复后的操作必须真的执行：{body}"
    );
    assert_eq!(body["started"], true, "恢复后的操作必须真的执行：{body}");
    assert_eq!(body["exitCode"], 0, "成功夹具以退出码 0 结束：{body}");
    assert_eq!(
        invokes(&tree, "success").len(),
        1,
        "恢复后的请求必须产生一次真实调用：{:#?}",
        tree.records()
    );

    // 断线不产生结果恢复、重放或补做：静置后不得再出现任何新的调用。
    tokio::time::sleep(QUIET_PERIOD).await;
    assert_eq!(
        invokes(&tree, "long-running").len(),
        1,
        "断线那次操作不得被重放或补做：{:#?}",
        tree.records()
    );
    assert_eq!(
        tree.records().len(),
        4,
        "连接断开只留一次完整调用，随后一次恢复调用：{:#?}",
        tree.records()
    );
}

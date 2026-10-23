//! 不自动超时终止、不提前释放执行资格（契约 P12）：脚本未确认结束时，后端不因经过任何时间阈值
//! 自动终止脚本，也不提前释放执行资格。
//!
//! 验证方式：把 `long-running.bat` 绑定为“启动”目标并真实调用它，然后在**明显长于任何合理默认
//! 超时窗口**（常见的 30s / 60s）的等待期间，反复经**独立 TCP 连接**提交两个固定操作，断言：
//!
//! - 每次提交都在极短时间内得到 `409 operation-in-progress`（既没有等待脚本结束，也没有变成执行）；
//! - 夹具进程自始至终是同一个 PID 且一直存活（没有被强杀、也没有被重启）；
//! - 调用记录里 `long-running` 只有一条 `invoke`，等待期间没有任何结束记录、也没有任何其他调用；
//! - 脚本只能由人工结束入口按它自己的方式结束，之后资格才释放并恢复接受操作。
//!
//! “后端没有强杀”由进程存活这一真实观测给出：后端若终止过脚本，`cmd.exe` 立即消失，本测试的存活
//! 断言与会话结束方式都会失败。执行生命周期里也不存在任何超时、强杀或定时释放资格的路径，
//! 这一点另由策略检查证明（见根 `README.md` 的“没有自动终止策略”一节）。
//!
//! 验证限度：有限等待（此处 75s）本身**不能**证明“永不超时”，它证明的是这段时间内不存在任何阈值
//! 行为；永不超时由“代码中不存在该策略”的检查补强，两者合起来才是本票的结论。

mod common;

use std::time::{Duration, Instant};

use common::{
    CallRecord, FixtureTree, HttpResponse, OpenRequest, TestServer, config, post_over_tcp,
};

/// 忙碌结果的编码（见根 `README.md` 的“操作进行中”一节）。
const BUSY_STATUS: u16 = 409;
const BUSY_CODE: &str = "operation-in-progress";

/// “启动”与“停止”操作的请求路径。
const START: &str = "/api/operations/start";
const STOP: &str = "/api/operations/stop";

/// 等待检查点：从脚本调用被接受起，在这些时刻检查“后端仍忙碌、脚本仍在跑”。
///
/// 最后一个检查点同时也是本测试断言已跨过的等待时长：75s 明显长于任何合理默认超时窗口
/// （常见的 30s / 60s），也长于本仓库测试与夹具自身使用的任何等待。
const CHECKPOINTS_SECONDS: [u64; 5] = [15, 30, 45, 60, 75];

/// 断言“等待必须明显长于任何合理时间阈值”的下限，等于最后一个检查点。
const HOLD_DEADLINE: Duration = Duration::from_secs(75);

/// 单次探测请求的耗时上限：超过它说明请求被执行中的脚本挡住（等待脚本结束），
/// 而不是立即得到忙碌结果。
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// 两个操作各自的探测都算“立即返回”的耗时上限。执行中的脚本还要 15s 以上才会结束，
/// 因此“忙碌而不是等待”与“被挡住”之间有足够大的间隔。
const BUSY_BUDGET: Duration = Duration::from_secs(5);

/// 等待夹具记录/进程状态变化的上限。
const RECORD_DEADLINE: Duration = Duration::from_secs(30);

/// 结束入口后等待夹具进程消失的上限（结束脚本自身还要走它的宽限期）。
const END_DEADLINE: Duration = Duration::from_secs(60);

/// 操作结束后的静置时间：足够让“超时造成的重放、补做或自行结束”暴露出来。
const QUIET_PERIOD: Duration = Duration::from_secs(3);

/// 结束入口的宽限秒数：夹具每 200ms 轮询一次停止标记，60s 宽限足以让“夹具按停止标记自行结束”
/// 保持可观察，不会落到结束脚本的兜底强杀（后者会掩盖“谁结束了脚本”这一点）。
const STOP_GRACE_SECONDS: u32 = 60;

/// 提交一次操作请求并限定耗时：请求挂住说明它被执行中的脚本挡住了（或真的开始了一次新的执行），
/// 而本票要求的观察点是“立即返回的忙碌结果”，因此挂住本身就是失败。
async fn probe(addr: std::net::SocketAddr, path: &str, deadline: Duration) -> HttpResponse {
    match tokio::time::timeout(deadline, post_over_tcp(addr, path)).await {
        Ok(response) => response,
        Err(_) => panic!(
            "{path} 必须在 {deadline:?} 内返回：挂住说明这次请求被执行中的操作挡住或开始了一次新的执行，\
             而脚本尚未确认结束时的唯一合法结果是立即返回的忙碌结果"
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
        "{path} 在执行期间必须得到忙碌结果：{response:?}"
    );
    let body: serde_json::Value =
        serde_json::from_str(&response.body).expect("忙碌响应必须是 JSON");
    assert_eq!(
        body["code"], BUSY_CODE,
        "{path} 的忙碌类别必须可区分：{body}"
    );
    assert!(
        body.get("outcome").is_none(),
        "{path} 的忙碌响应不得伪装成一次执行结果：{body}"
    );
}

/// “启动”绑定长跑夹具（只有停止标记能让它结束），“停止”绑定立即成功结束的夹具：
/// 于是“人工结束后恢复接受操作”能用另一个夹具的真实调用记录证明。
fn config_for(tree: &FixtureTree) -> herdr_bot::Config {
    let mut config = config(&tree.script("long-running.bat"), &tree.operations());
    config.operations.stop.script = tree.script("success.bat");
    config
}

/// 长跑脚本未确认结束时，后端不自动终止脚本，也不提前释放执行资格；人工结束后资格才释放。
#[tokio::test(flavor = "multi_thread")]
async fn held_script_is_not_auto_terminated_and_execution_stays_busy() {
    let tree = FixtureTree::new("no-auto-termination");
    let server = TestServer::start(&config_for(&tree)).await;

    // 提交“启动”后不读响应：长跑夹具不会自行结束，这条连接代表一次仍在等待结果的操作。
    let waiting_client = OpenRequest::submit(server.addr(), START).await;

    // 正对照：这次请求确实已被接受，并产生一次真实调用，夹具正在运行。
    let accepted = common::wait_until(RECORD_DEADLINE, || {
        !invokes(&tree, "long-running").is_empty()
    })
    .await;
    assert!(
        accepted,
        "正对照：提交后必须产生一次 long-running 调用记录：{:#?}",
        tree.records()
    );
    let invoked = invokes(&tree, "long-running").remove(0);
    assert!(
        common::process_is_alive(invoked.pid),
        "等待开始前夹具必须正在运行（PID {}）",
        invoked.pid
    );
    let started = Instant::now();

    // 长时间等待：在每个检查点经独立 TCP 连接提交两个操作，两个操作都必须立即得到忙碌结果。
    for seconds in CHECKPOINTS_SECONDS {
        tokio::time::sleep(Duration::from_secs(seconds).saturating_sub(started.elapsed())).await;
        let at = started.elapsed();

        let probing = Instant::now();
        let busy_start = probe(server.addr(), START, PROBE_DEADLINE).await;
        let busy_stop = probe(server.addr(), STOP, PROBE_DEADLINE).await;
        let probe_elapsed = probing.elapsed();
        assert_busy(START, &busy_start);
        assert_busy(STOP, &busy_stop);
        assert!(
            probe_elapsed < BUSY_BUDGET,
            "t+{at:?} 的重叠请求必须立即返回忙碌结果（耗时 {probe_elapsed:?}）"
        );

        // 脚本没有被自动终止：同一个 PID 仍在运行，没有被强杀，也没有被重新启动。
        assert!(
            common::process_is_alive(invoked.pid),
            "t+{at:?} 后端不得自动终止脚本：夹具必须仍在运行（PID {}）",
            invoked.pid
        );
        // 资格仍被持有：记录里只有最初那一次调用，没有第二次推进，也没有任何结束记录。
        assert_eq!(
            invokes(&tree, "long-running").len(),
            1,
            "t+{at:?} 不得出现第二次调用或重启：{:#?}",
            tree.records()
        );
        assert_eq!(
            records_of(&tree, "long-running").len(),
            1,
            "t+{at:?} 脚本未确认结束前不得出现结束记录（后端不得强杀它）：{:#?}",
            tree.records()
        );
        assert_eq!(
            invokes(&tree, "success").len(),
            0,
            "t+{at:?} 忙碌请求不得执行任何脚本：{:#?}",
            tree.records()
        );
        assert_eq!(
            tree.records().len(),
            1,
            "t+{at:?} 等待期间不得出现任何其他调用记录：{:#?}",
            tree.records()
        );

        println!(
            "[no-auto-termination] t+{at:?}: start={} stop={} pid{}={} records={}",
            busy_start.status,
            busy_stop.status,
            invoked.pid,
            common::process_is_alive(invoked.pid),
            tree.records().len()
        );
    }

    // 断言真的等满了这段时长：等待本身必须明显长于任何合理默认超时窗口。
    assert!(
        started.elapsed() >= HOLD_DEADLINE,
        "必须等待明显长于任何合理时间阈值（{:?}），实际 {:?}",
        HOLD_DEADLINE,
        started.elapsed()
    );

    // 人工结束入口（与人工处理用的是同一个脚本）：先写停止标记让夹具按自己的方式结束。
    let stop_output = tree.stop_fixtures_with_grace(STOP_GRACE_SECONDS);
    assert!(
        stop_output.status.success(),
        "结束脚本必须成功退出（表示无遗留进程）：{stop_output:?}"
    );
    assert!(
        common::wait_until(END_DEADLINE, || !common::process_is_alive(invoked.pid)).await,
        "结束后不得遗留运行中的夹具进程（PID {}）",
        invoked.pid
    );

    // 是夹具自己看到停止标记后结束的，不是结束脚本的兜底强杀：也没有任何后端自动终止的痕迹。
    let long_running = records_of(&tree, "long-running");
    assert_eq!(
        long_running.len(),
        2,
        "一次调用必须留下开始与结束两条记录：{long_running:#?}"
    );
    assert_eq!(long_running[0].stage, "invoke");
    assert_eq!(
        long_running[1].stage, "stop-observed",
        "夹具必须自己看到停止标记后才结束（否则说明它早已不在）：{long_running:#?}"
    );
    assert_eq!(
        long_running[0].call_id, long_running[1].call_id,
        "开始与结束必须属于同一次调用：{long_running:#?}"
    );
    assert_eq!(long_running[0].call, long_running[1].call);
    assert_eq!(long_running[0].pid, long_running[1].pid);
    let stop_record = records_of(&tree, "stop-fixtures");
    assert_eq!(stop_record.len(), 1, "结束脚本必须留下一条记录");
    assert!(
        stop_record[0]
            .detail
            .contains("long-running=ended-by-stop-marker"),
        "夹具必须由停止标记按自己的方式结束：{}",
        stop_record[0].detail
    );
    assert!(
        stop_record[0].detail.contains("detached-child=not-running"),
        "结束脚本必须确认没有其它遗留夹具进程：{}",
        stop_record[0].detail
    );

    // 脚本确认结束后资格才释放并恢复接受操作：下一次“停止”被接受并真的执行。
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

    // 等待期间没有超时造成的重放、补做或自行结束：静置后记录不再增加。
    tokio::time::sleep(QUIET_PERIOD).await;
    assert_eq!(
        invokes(&tree, "long-running").len(),
        1,
        "长跑脚本不得被重放或补做：{:#?}",
        tree.records()
    );
    assert_eq!(
        tree.records().len(),
        4,
        "只留一次完整的长跑调用与随后一次恢复调用：{:#?}",
        tree.records()
    );

    // 那条一直等待结果的连接此刻才关掉：操作早已结束，断线不再影响任何东西。
    drop(waiting_client);
    assert_eq!(invokes(&tree, "success").len(), 1, "断线不得补做或重放操作");
}

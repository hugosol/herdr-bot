//! 票 08（契约 P9）在 S2（HTTP 操作 interface）接缝上的验收测试：
//! 操作结束后的再次提交，以及“重复启动不做运行状态去重”。
//!
//! 断言的是两件可观察的事：
//!
//! 1. **结束后资格释放**：脚本成功或失败结束后，后端接受下一次操作，并**真的再次调用**配置绑定的脚本。
//!    这里的“连续提交”指前一次的 HTTP 响应已经返回（响应里 `ended: true` 且带 `outcome`）之后才发出
//!    下一次请求，因此两个请求在时间上不重叠——若下一次请求得到 `409 operation-in-progress`，那只能是
//!    “结束后的提交被误判为重叠”，而不是真的并发（并发情形由 `mutual_exclusion.rs` 覆盖）。
//! 2. **不做运行状态去重**：连续两次“启动”产生**两次**启动脚本调用。后端不检查服务是否已在运行，
//!    也不因为上一次启动而跳过脚本调用。
//!
//! 判定依据是夹具自己写下的调用记录（`fixtures/README.md`）：某个夹具的 `invoke` 行数就是它被真实调用的
//! 次数。期望来自独立给定的夹具情形（`success.bat` 以 0 结束、`failure.bat` 以 3 结束），不复算后端的
//! 分类逻辑；每个测试把 `fixtures/` 复制到独立临时目录，因此并行运行不互相污染记录。

mod common;

use std::net::SocketAddr;

use common::{CallRecord, FixtureTree, TestServer, config, post_over_tcp};
use serde_json::Value;

/// “启动”操作的请求路径。
const START: &str = "/api/operations/start";
/// “停止”操作的请求路径。
const STOP: &str = "/api/operations/stop";

/// 忙碌结果的编码：结束后的合法提交**不得**落到这个类别。
const BUSY_STATUS: u16 = 409;
const BUSY_CODE: &str = "operation-in-progress";

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

/// 提交一次操作并等它**结束**，返回结果体；调用返回时本次操作的响应已经返回。
///
/// 这是本票“连续提交”的定义：只有在拿到前一次的结束结果之后，测试才会发出下一次请求。
/// 因此任何 `409 operation-in-progress` 都说明“结束后的提交被误判为重叠”，而不是真实并发。
/// 每一步都断言这不是拒绝响应，也不是伪装成结果的忙碌响应。
async fn submit_ended(addr: SocketAddr, path: &str, expected_outcome: &str) -> Value {
    let response = post_over_tcp(addr, path).await;

    let body: Value = serde_json::from_str(&response.body)
        .unwrap_or_else(|_| panic!("{path} 的操作响应必须是 JSON：{response:?}"));

    assert!(
        response.status != BUSY_STATUS,
        "{path} 是上一次操作**结束后**才发出的提交，不得被误判为重叠请求而拒绝：{response:?}"
    );
    assert_eq!(
        response.status, 200,
        "{path} 在操作结束后必须被接受并返回执行结果：{response:?}"
    );
    assert!(
        body.get("code").is_none(),
        "{path} 不得返回拒绝类别（忙碌或其他拒绝）：{body}"
    );
    assert_eq!(
        body["ended"], true,
        "{path} 的响应必须表示本次调用已结束：{body}"
    );
    assert_eq!(
        body["outcome"], expected_outcome,
        "{path} 的结果类别来自独立给定的夹具情形：{body}"
    );
    body
}

/// 成功结束（`success.bat` 退出码 0）后，下一次操作被接受并真实执行。
#[tokio::test]
async fn success_end_accepts_the_next_operation_and_it_executes() {
    let tree = FixtureTree::new("next-op-after-success");
    let server = TestServer::start(&config_with(&tree, "success.bat", "failure.bat")).await;
    let addr = server.addr();

    // 第一次操作：成功结束，响应已返回（`ended` + `outcome` 见 `submit_ended`）。
    let body = submit_ended(addr, START, "completed").await;
    assert_eq!(body["started"], true, "启动脚本必须真的被执行：{body}");
    assert_eq!(body["exitCode"], 0, "成功退出码来自夹具自身：{body}");
    assert_eq!(
        invokes(&tree, "success").len(),
        1,
        "第一次操作必须产生一次启动脚本调用：{:#?}",
        tree.records()
    );

    // 下一次操作：与上一次不重叠，必须被接受并真实执行另一个固定脚本。
    let body = submit_ended(addr, STOP, "failed").await;
    assert_eq!(
        body["started"], true,
        "结束后的下一次操作必须真的执行脚本：{body}"
    );
    assert_eq!(body["exitCode"], 3, "失败退出码来自夹具自身：{body}");
    assert_eq!(
        body["failure"], "failed-exit",
        "失败子类别必须可区分：{body}"
    );
    assert_eq!(
        invokes(&tree, "failure").len(),
        1,
        "结束后的“停止”必须产生一次新的调用记录：{:#?}",
        tree.records()
    );
    assert_eq!(
        invokes(&tree, "success").len(),
        1,
        "下一次操作不得重跑上一个操作：{:#?}",
        tree.records()
    );
}

/// 失败结束（`failure.bat` 退出码 3）后，紧接着提交下一次操作**不得**被拒绝为忙碌，且真实执行。
#[tokio::test]
async fn failure_end_accepts_the_next_operation_and_it_executes() {
    let tree = FixtureTree::new("next-op-after-failure");
    let server = TestServer::start(&config_with(&tree, "failure.bat", "success.bat")).await;
    let addr = server.addr();

    // 第一次操作：失败结束（退出码 3）。
    let body = submit_ended(addr, START, "failed").await;
    assert_eq!(
        body["started"], true,
        "失败退出也意味着脚本真的被执行：{body}"
    );
    assert_eq!(body["exitCode"], 3, "失败退出码来自夹具自身：{body}");
    assert_eq!(
        body["failure"], "failed-exit",
        "失败子类别必须是退出失败：{body}"
    );
    assert_eq!(
        invokes(&tree, "failure").len(),
        1,
        "第一次操作必须产生一次调用记录：{:#?}",
        tree.records()
    );

    // 紧接着（前一次响应已返回）的下一次操作：失败结束同样释放资格。
    let body = submit_ended(addr, STOP, "completed").await;
    assert_eq!(
        body["started"], true,
        "失败结束后的下一次操作必须真的执行脚本：{body}"
    );
    assert_eq!(body["exitCode"], 0, "成功退出码来自夹具自身：{body}");
    assert_eq!(
        invokes(&tree, "success").len(),
        1,
        "失败结束后提交的“停止”必须产生一次新的调用记录：{:#?}",
        tree.records()
    );

    // 失败结束后的再一次“启动”同样按脚本调用处理，不因为“可能已经在跑”而跳过。
    let body = submit_ended(addr, START, "failed").await;
    assert_eq!(
        body["started"], true,
        "失败结束后的再次启动必须真的执行脚本：{body}"
    );
    assert_eq!(
        invokes(&tree, "failure").len(),
        2,
        "失败结束后的再次启动必须产生第二次调用记录（不做运行状态去重）：{:#?}",
        tree.records()
    );
}

/// 连续两次“启动”产生**两次**启动脚本调用：不存在“已运行则跳过”的行为。
#[tokio::test]
async fn consecutive_starts_invoke_the_start_script_twice() {
    let tree = FixtureTree::new("consecutive-starts");
    let server = TestServer::start(&config_with(&tree, "success.bat", "failure.bat")).await;
    let addr = server.addr();

    let first = submit_ended(addr, START, "completed").await;
    assert_eq!(
        first["started"], true,
        "第一次启动必须真的执行脚本：{first}"
    );
    assert_eq!(
        invokes(&tree, "success").len(),
        1,
        "第一次启动必须产生一次调用记录：{:#?}",
        tree.records()
    );

    // 第二次“启动”发生在前一次响应返回之后（上一次脚本已经结束），仍然是脚本调用。
    let second = submit_ended(addr, START, "completed").await;
    assert_eq!(
        second["started"], true,
        "第二次启动必须真的执行脚本：{second}"
    );
    assert_eq!(
        second["exitCode"], 0,
        "第二次启动同样以夹具的成功退出码结束：{second}"
    );

    let runs = invokes(&tree, "success");
    assert_eq!(
        runs.len(),
        2,
        "连续两次“启动”必须产生两次启动脚本调用，而不是“已运行则跳过”：{:#?}",
        tree.records()
    );
    assert_ne!(
        runs[0].call_id, runs[1].call_id,
        "两次启动必须是两次真实调用：{runs:#?}"
    );
    assert_ne!(
        runs[0].seq, runs[1].seq,
        "两次启动必须是两条记录行：{runs:#?}"
    );
    assert!(
        !common::process_is_alive(runs[1].pid),
        "第二次启动返回时它的脚本进程应当已经结束（PID {}）",
        runs[1].pid
    );
}

/// “启动”与“停止”可以连续交替使用：每次结束之后的下一次操作都被接受并真实执行。
#[tokio::test]
async fn start_and_stop_alternate_across_several_finished_operations() {
    let tree = FixtureTree::new("alternate-operations");
    let server = TestServer::start(&config_with(&tree, "success.bat", "failure.bat")).await;
    let addr = server.addr();

    // 启动（成功）→ 停止（失败）→ 启动（成功）→ 停止（失败）。
    let chain = [
        (START, "completed"),
        (STOP, "failed"),
        (START, "completed"),
        (STOP, "failed"),
    ];
    for (index, (path, outcome)) in chain.iter().enumerate() {
        let body = submit_ended(addr, path, outcome).await;
        assert_eq!(
            body["started"],
            true,
            "链条第 {} 次操作必须真的执行脚本：{body}",
            index + 1
        );
        let expected_success = chain[..=index]
            .iter()
            .filter(|(path, _)| *path == START)
            .count();
        let expected_failure = chain[..=index]
            .iter()
            .filter(|(path, _)| *path == STOP)
            .count();
        assert_eq!(
            invokes(&tree, "success").len(),
            expected_success,
            "第 {} 次操作后“启动”的调用次数：{:#?}",
            index + 1,
            tree.records()
        );
        assert_eq!(
            invokes(&tree, "failure").len(),
            expected_failure,
            "第 {} 次操作后“停止”的调用次数：{:#?}",
            index + 1,
            tree.records()
        );
    }
}

/// 结束后的提交不会与“上一次操作仍在执行”混淆：任何一次提交都在前一次响应返回之后，
/// 因此这里每个请求都必须是执行结果（200 + `outcome`），不能出现忙碌类别。
#[tokio::test]
async fn repeated_submissions_never_fall_into_the_busy_class() {
    let tree = FixtureTree::new("no-false-busy");
    let server = TestServer::start(&config_with(&tree, "success.bat", "failure.bat")).await;
    let addr = server.addr();

    // 三轮“启动后紧跟一次停止”，每次都等前一次结束。
    for round in 1..=3 {
        let response = post_over_tcp(addr, START).await;
        assert_eq!(
            response.status, 200,
            "第 {round} 轮启动在操作结束后必须被接受：{response:?}"
        );
        let body: Value = serde_json::from_str(&response.body).expect("操作响应必须是 JSON");
        assert_eq!(
            body["ended"], true,
            "第 {round} 轮启动的响应必须已结束：{body}"
        );

        let response = post_over_tcp(addr, STOP).await;
        assert!(
            response.status != BUSY_STATUS,
            "第 {round} 轮停止不得得到忙碌结果：{response:?}"
        );
        assert_eq!(
            response.status, 200,
            "第 {round} 轮停止必须被接受：{response:?}"
        );
        let body: Value = serde_json::from_str(&response.body).expect("操作响应必须是 JSON");
        assert_ne!(
            body["code"], BUSY_CODE,
            "第 {round} 轮停止不得是忙碌类别：{body}"
        );
    }

    assert_eq!(
        invokes(&tree, "success").len(),
        3,
        "三轮启动各执行一次：{:#?}",
        tree.records()
    );
    assert_eq!(
        invokes(&tree, "failure").len(),
        3,
        "三轮停止各执行一次：{:#?}",
        tree.records()
    );
    assert_eq!(
        tree.records().len(),
        6,
        "六次操作各留下恰好一条调用记录，没有多余调用：{:#?}",
        tree.records()
    );
}

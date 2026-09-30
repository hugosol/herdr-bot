//! 隔离批处理夹具的验收测试：经服务端配置把这些夹具绑定为操作目标，用真实 HTTP 请求驱动，
//! 再核对夹具自己写下的调用记录。
//!
//! 这些检查证明的是夹具行为与调用记录机制（每类情形可被真实调用、记录与调用一一对应），
//! 不替代后续票对后端生命周期与结果分类的验收。

mod common;

use std::time::Duration;

use common::{FixtureTree, RECORD_FILE, config, post_start, same_path};

#[tokio::test]
async fn success_fixture_records_one_call_and_ends_immediately_with_zero() {
    let tree = FixtureTree::new("success");
    let app = omp_bot::app(&config(&tree.script("success.bat"), &tree.operations()));

    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["ended"], true);
    assert_eq!(body["started"], true);
    assert_eq!(body["exitCode"], 0, "成功夹具以退出码 0 结束");

    let records = tree.records();
    assert_eq!(
        records.len(),
        1,
        "一次调用只写入一条记录（文件 {}）：{records:#?}",
        tree.root().join(RECORD_FILE).display()
    );
    let record = &records[0];
    assert_eq!(record.fixture, "success");
    assert_eq!(record.stage, "invoke", "调用记录以 invoke 阶段开始");
    assert_eq!(record.call, 1, "首次调用的调用序号为 1");
    assert_eq!(record.seq, 1, "记录行序号从 1 开始");
    assert!(record.pid > 0, "记录必须包含夹具进程 PID");
    assert!(
        same_path(&record.cwd, &tree.operations()),
        "记录必须包含夹具进程的实际工作目录：{}",
        record.cwd.display()
    );
    assert!(
        record.timestamp.contains('T') && record.timestamp.len() >= 19,
        "记录必须包含可排序的时间戳：{}",
        record.timestamp
    );

    // “仅一次调用”与“无延迟补做”：静置一段时间后记录不得增加。
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(tree.records().len(), 1, "调用结束后不得出现延迟补做的记录");
}

#[tokio::test]
async fn success_fixture_process_is_gone_after_the_call() {
    let tree = FixtureTree::new("success-gone");
    let app = omp_bot::app(&config(&tree.script("success.bat"), &tree.operations()));

    let (_, body) = post_start(app).await;
    assert_eq!(body["exitCode"], 0);

    let pid = tree.records()[0].pid;
    assert!(
        !common::process_is_alive(pid),
        "立即结束的夹具不得在验证后遗留进程（PID {pid}）"
    );
}

#[tokio::test]
async fn failure_fixture_records_one_call_and_ends_with_explicit_failure_code() {
    let tree = FixtureTree::new("failure");
    let app = omp_bot::app(&config(&tree.script("failure.bat"), &tree.operations()));

    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["ended"], true);
    assert_eq!(body["started"], true);
    assert_eq!(
        body["exitCode"], 3,
        "失败夹具以它自己声明的失败退出码结束（fixtures/operations/failure.bat）"
    );

    let records = tree.records();
    assert_eq!(records.len(), 1, "一次调用只写入一条记录：{records:#?}");
    assert_eq!(records[0].fixture, "failure");
    assert_eq!(records[0].stage, "invoke");
    assert_eq!(records[0].call, 1);
    assert!(
        same_path(&records[0].cwd, &tree.operations()),
        "记录必须包含夹具进程的实际工作目录：{}",
        records[0].cwd.display()
    );
    assert_eq!(
        body["exitCode"].as_i64(),
        records[0]
            .detail
            .trim_start_matches("exit-code=")
            .parse::<i64>()
            .ok(),
        "夹具自报的退出码必须与后端观测到的退出码一致：{}",
        records[0].detail
    );
}

/// “无法启动”情形之一：配置指向的脚本不存在。
///
/// 夹具从未运行过，因此既不得写入调用记录，也不得给出成功形状的结果；
/// 把这种结果分类成“无法启动”由后续票在结果分类中定义。
#[tokio::test]
async fn unstartable_missing_script_never_runs_and_is_not_reported_as_success() {
    let tree = FixtureTree::new("unstartable-missing");
    let missing = tree.script("unstartable/missing-script.bat");
    assert!(!missing.exists(), "夹具前提：该目标确实不存在");

    let app = omp_bot::app(&config(&missing, &tree.operations()));
    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["ended"], true);
    assert_ne!(
        body["exitCode"],
        serde_json::json!(0),
        "目标不存在时不得被当作成功结束"
    );
    assert_eq!(
        tree.records().len(),
        0,
        "目标不存在时不得写入任何调用记录（夹具没有被执行）"
    );
}

/// “无法启动”情形之二：配置指向的目标存在但不是可执行文件。
#[tokio::test]
async fn unstartable_invalid_target_never_runs_and_is_not_reported_as_success() {
    let tree = FixtureTree::new("unstartable-invalid");
    let target = tree.script("unstartable/invalid-target.exe");
    assert!(target.is_file(), "夹具前提：该目标文件存在");

    let app = omp_bot::app(&config(&target, &tree.operations()));
    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["ended"], true);
    assert_ne!(
        body["exitCode"],
        serde_json::json!(0),
        "目标不可执行时不得被当作成功结束"
    );
    assert_eq!(
        tree.records().len(),
        0,
        "不可执行的目标不会运行，因此不得写入调用记录"
    );
}

/// “分离长期子进程后立即退出”夹具：完成判定不得等待另行启动的长期进程。
#[tokio::test(flavor = "multi_thread")]
async fn detached_child_keeps_running_after_the_call_returns() {
    let tree = FixtureTree::new("spawn-detached");
    let app = omp_bot::app(&config(&tree.script("spawn-detached.bat"), &tree.operations()));

    let started_at = std::time::Instant::now();
    let (status, body) = tokio::time::timeout(Duration::from_secs(30), common::post_start(app))
        .await
        .expect("分离子进程长期运行，父夹具的调用必须立即返回");
    let elapsed = started_at.elapsed();
    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["started"], true);
    assert_eq!(body["exitCode"], 0, "父夹具启动子进程后立即以 0 结束");
    assert!(
        elapsed < Duration::from_secs(15),
        "调用不得等待分离子进程（耗时 {elapsed:?}）"
    );

    let spawn_records: Vec<common::CallRecord> = tree
        .records()
        .into_iter()
        .filter(|record| record.fixture == "spawn-detached")
        .collect();
    assert_eq!(spawn_records.len(), 1, "父夹具只写入一次调用记录");
    assert_eq!(spawn_records[0].stage, "invoke");

    // 子进程自行登记；父调用已经返回，而它仍在运行。
    let registered = common::wait_until(Duration::from_secs(20), || {
        tree.records()
            .iter()
            .any(|record| record.fixture == "detached-child")
    })
    .await;
    assert!(
        registered,
        "分离子进程必须留下可识别的登记记录：{:#?}",
        tree.records()
    );
    let child = tree
        .records()
        .into_iter()
        .find(|record| record.fixture == "detached-child")
        .expect("子进程登记记录");
    assert_eq!(child.stage, "child-start");
    assert!(
        common::process_is_alive(child.pid),
        "父调用结束后分离子进程必须仍在运行（PID {}）",
        child.pid
    );

    // 子进程可识别、可结束，验证后不遗留。
    let stop_output = tree.stop_fixtures();
    assert!(
        stop_output.status.success(),
        "结束脚本必须成功退出：{stop_output:?}"
    );
    assert!(
        common::wait_until(Duration::from_secs(15), || !common::process_is_alive(
            child.pid
        ))
        .await,
        "分离子进程必须被结束（PID {}）",
        child.pid
    );

    let records = tree.records();
    let child_records: Vec<&common::CallRecord> = records
        .iter()
        .filter(|record| record.fixture == "detached-child")
        .collect();
    assert_eq!(
        child_records
            .iter()
            .map(|record| record.stage.as_str())
            .collect::<Vec<_>>(),
        vec!["child-start", "stop-observed"],
        "子进程的开始与结束都必须被记录：{records:#?}"
    );
    assert_eq!(
        child_records[0].call, child_records[1].call,
        "子进程的两条记录属于同一次调用"
    );
    let stop_record = records
        .iter()
        .find(|record| record.fixture == "stop-fixtures")
        .expect("结束脚本必须留下记录");
    assert!(
        stop_record.detail.contains("detached-child="),
        "结束方式必须随夹具记录：{}",
        stop_record.detail
    );
}

/// 记录机制：重叠调用下追加写不丢行、调用序号互相区分、时间戳可排序。
#[tokio::test(flavor = "multi_thread")]
async fn overlapping_calls_are_recorded_once_each_without_lost_lines() {
    const CALLS: usize = 6;
    let tree = FixtureTree::new("overlap");
    let script = tree.script("success.bat");
    let working_directory = tree.operations();

    let mut tasks = Vec::new();
    for _ in 0..CALLS {
        let script = script.clone();
        let working_directory = working_directory.clone();
        tasks.push(tokio::spawn(async move {
            common::run_batch(&script, &working_directory, &[]).await
        }));
    }
    for task in tasks {
        assert_eq!(
            task.await.expect("并发调用任务"),
            Some(0),
            "每个重叠调用都必须以 0 结束"
        );
    }

    let records = tree.records();
    assert_eq!(
        records.len(),
        CALLS,
        "{CALLS} 次重叠调用必须留下 {CALLS} 条记录：{records:#?}"
    );
    assert!(
        records
            .iter()
            .all(|record| record.fixture == "success" && record.stage == "invoke"),
        "每条记录都对应一次真实调用：{records:#?}"
    );
    assert!(
        records.iter().all(|record| record.pid > 0),
        "每条记录都必须带上夹具进程 PID"
    );

    let mut calls: Vec<u64> = records.iter().map(|record| record.call).collect();
    calls.sort_unstable();
    assert_eq!(
        calls,
        (1..=CALLS as u64).collect::<Vec<_>>(),
        "重叠调用的调用序号必须互相区分：{records:#?}"
    );

    let mut seqs: Vec<u64> = records.iter().map(|record| record.seq).collect();
    seqs.sort_unstable();
    assert_eq!(
        seqs,
        (1..=CALLS as u64).collect::<Vec<_>>(),
        "记录行序号必须连续无缺口（追加写不丢行）：{records:#?}"
    );

    let mut sorted = records.clone();
    sorted.sort_by_key(|record| record.seq);
    let timestamps: Vec<&str> = sorted.iter().map(|r| r.timestamp.as_str()).collect();
    let mut ascending = timestamps.clone();
    ascending.sort_unstable();
    assert_eq!(
        timestamps, ascending,
        "按行序号排序后时间戳不得倒退（记录可排序）：{records:#?}"
    );

    let call_ids: std::collections::HashSet<&str> = records
        .iter()
        .map(|record| record.call_id.as_str())
        .collect();
    assert_eq!(
        call_ids.len(),
        CALLS,
        "每次调用的调用标识必须唯一：{records:#?}"
    );
}

/// 记录位置可配置：环境变量 OMP_BOT_FIXTURE_RECORD_DIR 决定记录目录。
#[tokio::test(flavor = "multi_thread")]
async fn record_location_can_be_pointed_at_another_directory() {
    let tree = FixtureTree::new("record-dir");
    let custom = common::TempDir::new("custom record dir");

    let status = common::run_batch(
        &tree.script("success.bat"),
        &tree.operations(),
        &[(
            "OMP_BOT_FIXTURE_RECORD_DIR",
            custom.path().to_str().expect("临时目录是有效路径"),
        )],
    )
    .await;
    assert_eq!(status, Some(0));

    let records = common::read_records(&custom.path().join("calls.tsv"));
    assert_eq!(records.len(), 1, "记录必须写入指定的记录目录");
    assert_eq!(records[0].fixture, "success");
    assert!(
        !tree.record_file().exists(),
        "指定记录目录后不得再写默认的 runtime 目录：{}",
        tree.record_file().display()
    );
}

/// 长跑夹具：启动后长时间不结束，且只能由人工/脚本入口结束。
///
/// 这里证明的是夹具行为（可被真实调用、持续运行、可由停止标记结束、被结束方式写进记录），
/// “后端不自动终止”由后续票在其上验收。
#[tokio::test(flavor = "multi_thread")]
async fn long_running_fixture_stays_running_until_the_stop_entry_point() {
    let tree = FixtureTree::new("long-running");
    let app = omp_bot::app(&config(&tree.script("long-running.bat"), &tree.operations()));

    // 后端请求在脚本结束前不会返回，因此放到后台任务里等待。
    let request = tokio::spawn(common::post_start(app));

    let recorded = common::wait_until(Duration::from_secs(20), || tree.records().len() == 1).await;
    assert!(recorded, "长跑夹具必须写入调用记录：{:#?}", tree.records());

    let invoke = tree.records().remove(0);
    assert_eq!(invoke.fixture, "long-running");
    assert_eq!(invoke.stage, "invoke");
    let pid = invoke.pid;
    assert!(
        common::process_is_alive(pid),
        "被调用后长跑夹具必须处于运行中（PID {pid}）"
    );

    // 不会因为时间流逝而结束。
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(
        common::process_is_alive(pid),
        "长跑夹具不得自行结束（PID {pid}）"
    );
    assert!(
        !request.is_finished(),
        "夹具未结束时后端请求不得提前返回"
    );

    // 人工结束入口：与人工结束时用的是同一个脚本。
    let stop_output = tree.stop_fixtures();
    assert!(
        stop_output.status.success(),
        "结束脚本必须成功退出：{stop_output:?}"
    );

    let (status, body) = tokio::time::timeout(Duration::from_secs(20), request)
        .await
        .expect("脚本结束后请求必须返回")
        .expect("后台请求任务");
    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["exitCode"], 0, "被停止标记结束的夹具以 0 结束");

    let records = tree.records();
    let stages: Vec<(&str, &str)> = records
        .iter()
        .map(|record| (record.fixture.as_str(), record.stage.as_str()))
        .collect();
    assert_eq!(
        stages,
        vec![
            ("long-running", "invoke"),
            ("long-running", "stop-observed"),
            ("stop-fixtures", "stop"),
        ],
        "记录必须与调用一一对应：{records:#?}"
    );
    assert_eq!(records[0].call, records[1].call, "同一次调用的两条记录共用调用序号");
    assert_eq!(records[0].call_id, records[1].call_id);
    assert!(records[1].seq > records[0].seq, "记录行序号必须递增");
    assert!(
        records[1].timestamp >= records[0].timestamp,
        "时间戳必须可排序：{} -> {}",
        records[0].timestamp,
        records[1].timestamp
    );
    assert!(
        records[2].detail.contains("long-running=ended-by-stop-marker"),
        "结束方式必须随夹具记录：{}",
        records[2].detail
    );

    assert!(
        !common::process_is_alive(pid),
        "验证后不得遗留运行中的进程（PID {pid}）"
    );
    assert!(
        !tree.root().join("runtime/stop/long-running.stop").exists(),
        "停止标记必须被消费，不能留到下一次调用"
    );
}

/// “无法启动”情形之三：工作目录不存在，进程根本无法创建。
#[tokio::test]
async fn unstartable_missing_working_directory_reports_not_started() {
    let tree = FixtureTree::new("unstartable-workdir");
    let missing_directory = tree.root().join("unstartable").join("missing-working-directory");

    let app = omp_bot::app(&config(&tree.script("success.bat"), &missing_directory));
    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["ended"], true);
    assert_eq!(body["started"], false, "工作目录不存在时进程无法创建");
    assert_eq!(body["exitCode"], serde_json::Value::Null);
    assert_eq!(
        tree.records().len(),
        0,
        "进程未创建时夹具不得被调用，也不得写入调用记录"
    );
}

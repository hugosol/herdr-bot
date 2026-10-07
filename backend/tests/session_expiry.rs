//! 在线／离线判定与过期扫描（`scan_expired` / `spawn_expiry_scan`）。
//!
//! 每个用例都经 [`TestServer`] 的真实监听端口写入，因此驱动的是交付运行时同一份路由与同一份会话表；
//! 会话表由用例自己持有，于是「过期扫描把在线翻为离线」这件事能同时从两侧观察：订阅到的可见变化（帧）
//! 与会话表本身（快照）。
//!
//! 过期**判定**只有一处（[`is_online`] 与 30s 常量），扫描只做一件事：把与判定不一致的旗标翻正并广播。
//! 前三个用例把时刻**注入** `scan_expired`，于是过期的观察不需要真的等 30s 过去，秒级完成；最后一个
//! 用例跑真实挂钟上的后台任务（判定窗口 30s 起、最迟 35s），因为「写入把离线行恢复在线」这件事只在
//! 真实时间下成立：视图的在线与否由当前时刻现算，注入的未来时刻只影响那一帧，不改变行的时间线。
//!
//! 覆盖：判定窗口内侧（含边界）不翻、超过窗口翻一次且不重复广播、离线不改写 agent 状态、离线行保留
//! （不老化、不因扫描移除）、`unregister` 是唯一的移除入口、omp-bot 重启（新的会话表）即清空，以及
//! 后台扫描任务把超时会话在 `last_seen` 起 ≤35s 内翻为离线、写入随后把它恢复在线。

mod common;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{HttpResponse, TestServer, post_json_over_tcp};
use omp_bot::observation::Observation;
use omp_bot::sessions::{
    EXPIRY_SCAN_INTERVAL, ONLINE_WINDOW, SessionView, SessionsState, spawn_expiry_scan,
};
use omp_bot::{PowerOff, PowerOffFailure};
use serde_json::json;
use tokio::sync::broadcast;

/// 过期扫描用例绝不接触关机路径；这个动作只会在路由接错时立刻把用例打失败。
struct NeverPowerOff;

impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        panic!("会话过期用例不应触发关机");
    }
}

/// 一个用例独占的服务：真实监听端口的应用，加上用例持有的同一份会话表与其订阅。
struct SessionServer {
    server: TestServer,
    sessions: Arc<SessionsState>,
    shutdown: Arc<omp_bot::countdown::ShutdownState>,
    frames: broadcast::Receiver<Observation>,
}

impl SessionServer {
    async fn start() -> Self {
        let config = common::config(Path::new("unused.bat"), Path::new("."));
        let sessions = Arc::new(SessionsState::new());
        let frames = sessions.observation().subscribe();
        // 与交付运行时同一份装配：后台维护循环要拿到的正是这里生成的关机句柄。
        let (app, state) =
            omp_bot::app_with_state(&config, Arc::new(NeverPowerOff), Arc::clone(&sessions));
        Self {
            server: TestServer::start_app(app).await,
            sessions,
            shutdown: state.shutdown,
            frames,
        }
    }

    /// 经真 TCP 提交一条会话事件。
    async fn post(&self, body: &str) -> HttpResponse {
        post_json_over_tcp(self.server.addr(), "/api/sessions/events", body).await
    }

    /// 以注入的时刻跑一遍过期扫描；时刻由调用方决定，因此过期的观察不需要真的等 30s。
    async fn scan(&self, now: Instant) {
        self.sessions.scan_expired(now).await;
    }

    /// 推进到「最后写入之后刚好超过判定窗口」再扫一遍。
    ///
    /// 注入的时刻是「此刻 + 30s + 1s」，而任何一次写入的 `last_seen` 都不晚于此刻，因此这一遍扫描
    /// 必然把该行判为离线。
    async fn scan_past_the_window(&self) {
        self.scan(Instant::now() + ONLINE_WINDOW + Duration::from_secs(1))
            .await;
    }

    /// 取走此刻已发布的全部帧。
    fn drain_frames(&mut self) -> Vec<Observation> {
        let mut frames = Vec::new();
        while let Ok(frame) = self.frames.try_recv() {
            frames.push(frame);
        }
        frames
    }

    /// 取走「本次写入/扫描必然已经发布」的那一帧：事件在返回响应之前就已广播，因此读到响应就必然
    /// 能收到。
    fn take_frame(&mut self) -> Observation {
        let mut frames = self.drain_frames();
        assert_eq!(frames.len(), 1, "本次应恰好发布一帧：{frames:?}");
        frames.remove(0)
    }

    /// 断言本次写入或扫描没有产生任何可见变化。
    fn assert_no_frame(&mut self) {
        let frames = self.drain_frames();
        assert!(frames.is_empty(), "不应发布任何帧：{frames:?}");
    }

    /// 当前全部会话的视图（与读路径首帧同一份快照，在线与否按**当前时刻**现算）。
    async fn snapshot(&self) -> Vec<SessionView> {
        self.sessions.snapshot().await
    }
}

/// 取出一帧 upsert 的会话视图。
fn upsert(frame: Observation) -> SessionView {
    let Observation::SessionUpsert(view) = frame else {
        panic!("应发布一帧会话 upsert：{frame:?}");
    };
    view
}

/// 断言一次成功回执（HTTP 200）。
fn assert_accepted(response: &HttpResponse) {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
}

fn register(id: &str) -> String {
    json!({ "sessionId": id, "kind": "register", "data": { "cwd": "C:/work/omp" } }).to_string()
}

fn heartbeat(id: &str) -> String {
    json!({ "sessionId": id, "kind": "heartbeat", "data": {} }).to_string()
}

fn state(id: &str, value: &str) -> String {
    json!({ "sessionId": id, "kind": "state", "data": { "value": value } }).to_string()
}

fn unregister(id: &str) -> String {
    json!({ "sessionId": id, "kind": "unregister" }).to_string()
}

/// 判定窗口内侧（含 30s 边界）不翻，超过窗口翻一次，且只翻一次：翻转后的行再扫（无论推进多远）
/// 都不重复广播。
///
/// 离线只改 keepAlive，不改写 agent 状态：翻转后的视图仍带着此前的状态。
#[tokio::test]
async fn an_expired_row_flips_to_offline_once_without_touching_its_state() {
    let mut server = SessionServer::start().await;
    // 写入不晚于这一刻，因此「此刻 + 30s」落在判定窗口内侧（含边界）。
    let before_write = Instant::now();
    assert_accepted(&server.post(&register("sess-flip")).await);
    let registered = upsert(server.take_frame());
    assert!(registered.online, "刚注册的行在线：{registered:?}");
    assert_accepted(&server.post(&state("sess-flip", "working")).await);
    let working = upsert(server.take_frame());
    assert_eq!(format!("{:?}", working.state), "Some(Working)");

    server.scan(before_write + ONLINE_WINDOW).await;
    server.assert_no_frame();

    server.scan_past_the_window().await;
    let offline = upsert(server.take_frame());
    assert_eq!(offline.session_id, "sess-flip");
    assert!(!offline.online, "超过窗口的行必须判为离线：{offline:?}");
    assert_eq!(
        format!("{:?}", offline.state),
        "Some(Working)",
        "离线只改 keepAlive，不改写 agent 状态"
    );

    // 已经翻过的行不重复广播：同一时刻再扫、把时刻推进到十分钟后也都不再发帧。
    server
        .scan(Instant::now() + ONLINE_WINDOW + Duration::from_secs(1))
        .await;
    server.scan(Instant::now() + Duration::from_secs(600)).await;
    server.assert_no_frame();
}

/// 任何被接受的写事件都把行自己的 `online` 旗标一并翻回在线。
///
/// 这个旗标就是扫描「这个值已经广播过」的依据：旗标与判定分叉，扫描就会在下一轮把它当成新变化再广播
/// 一帧。这里用注入时刻制造「旗标为离线、时间却在窗口内」的局面（真实时间下不可能构造），于是三种写
/// 事件各自都要在写入之后立刻让紧接着的扫描无话可说。
#[tokio::test]
async fn any_accepted_write_flips_the_rows_online_flag_back() {
    let mut server = SessionServer::start().await;
    assert_accepted(&server.post(&register("sess-flag")).await);
    server.take_frame();

    let writes = [
        ("heartbeat", heartbeat("sess-flag")),
        ("register", register("sess-flag")),
        ("state", state("sess-flag", "working")),
    ];
    for (kind, body) in writes {
        server.scan_past_the_window().await;
        let offline = upsert(server.take_frame());
        assert!(
            !offline.online,
            "{kind} 之前该行应已被判为离线：{offline:?}"
        );

        assert_accepted(&server.post(&body).await);
        // 写入自身可能发帧（例如状态真变化），先清掉；要紧的是随后的扫描。
        server.drain_frames();

        server.scan(Instant::now()).await;
        server.assert_no_frame();
    }
}

/// 离线行保留、不老化、不设上界：扫描不产生 `remove`，行只在显式 `unregister` 时消失。
///
/// omp-bot 重启（另一份全新的会话表）后该行不再存在——v1 不落盘。
#[tokio::test]
async fn an_offline_row_is_retained_until_unregister_and_a_restart_starts_empty() {
    let mut server = SessionServer::start().await;
    assert_accepted(&server.post(&register("sess-keep")).await);
    server.take_frame();

    server.scan_past_the_window().await;
    let offline = upsert(server.take_frame());
    assert!(!offline.online);

    // 翻过之后等十分钟也不会被老化掉：没有帧，行还在。
    server.scan(Instant::now() + Duration::from_secs(600)).await;
    server.assert_no_frame();
    let snapshot = server.snapshot().await;
    assert_eq!(snapshot.len(), 1, "离线行保留在表里：{snapshot:?}");
    assert_eq!(snapshot[0].session_id, "sess-keep");
    assert_eq!(snapshot[0].cwd, "C:/work/omp");

    // unregister 是唯一的移除入口：移除后行不再存在，扫描也不再对它有话可说。
    assert_accepted(&server.post(&unregister("sess-keep")).await);
    let Observation::SessionRemove { session_id } = server.take_frame() else {
        panic!("unregister 应发布一帧 remove");
    };
    assert_eq!(session_id, "sess-keep");
    server.scan_past_the_window().await;
    server.assert_no_frame();
    assert!(server.snapshot().await.is_empty(), "行已随 unregister 移除");

    // 另一份全新的会话表就是「omp-bot 重启」：内存态不落盘，重启即清空。
    let restarted = SessionServer::start().await;
    assert!(restarted.snapshot().await.is_empty(), "重启后没有残留会话");
    let response = restarted.post(&heartbeat("sess-keep")).await;
    assert_eq!(
        response.status, 404,
        "重启后旧会话是未知会话：{}",
        response.body
    );
}

/// 慢用例（判定窗口 30s 起、最迟 35s）：后台扫描任务由真实挂钟推进，无法用注入时刻覆盖。
///
/// 一次跑完在线／离线的整条生命周期：
/// - 最后一次写入起 ≤35s（判定窗口 30s + 扫描节奏 5s）出现离线的 `upsert`，且不得在 30s 内提前翻；
/// - 离线行保留在快照里（`online` 为假），不是被移除；
/// - 任何被接受的写入把离线行恢复在线，且这次 true 变化本身就是一次可见变化（广播一帧）；
/// - 写事件把行自己的 `online` 旗标一并翻正，因此紧接着的扫描不再发一帧重复的「恢复在线」；
/// - `unregister` 移除行，之后扫描不再有任何变化。
#[tokio::test]
async fn the_background_scan_flips_a_stale_session_offline_until_unregister() {
    let mut server = SessionServer::start().await;
    let before_register = Instant::now();
    assert_accepted(&server.post(&register("sess-slow")).await);
    let registered = upsert(server.take_frame());
    assert!(registered.online);

    spawn_expiry_scan(Arc::clone(&server.sessions), Arc::clone(&server.shutdown));

    // 判定窗口内不得翻转，因此等它必须等过 30s；上界给它 35s + 5s 余量，超时即以可读的方式失败。
    let frame = tokio::time::timeout(
        ONLINE_WINDOW + EXPIRY_SCAN_INTERVAL + Duration::from_secs(5),
        server.frames.recv(),
    )
    .await
    .expect("后台扫描应在 35s 内把超时会话翻为离线")
    .expect("会话表的广播通道不应关闭");
    let offline = upsert(frame);
    assert_eq!(offline.session_id, "sess-slow");
    assert!(!offline.online, "后台扫描必须翻为离线：{offline:?}");

    let elapsed = before_register.elapsed();
    assert!(
        elapsed > ONLINE_WINDOW,
        "不得在 30s 判定窗口内翻转：实际 {elapsed:?}"
    );
    assert!(
        elapsed <= ONLINE_WINDOW + EXPIRY_SCAN_INTERVAL + Duration::from_secs(1),
        "离线 upsert 的上界是 30s + 5s 节奏（另加 1s 调度余量）：实际 {elapsed:?}"
    );

    // 离线行保留在表里（此刻是真实时间，所以快照也如实说「离线」）。
    let stale = server.snapshot().await;
    assert_eq!(stale.len(), 1, "超时只翻 keepAlive，不移除行：{stale:?}");
    assert!(!stale[0].online, "过期行不得假「在线」：{stale:?}");

    // 一次写入把离线行恢复在线，且「恢复在线」本身就是可见变化。
    assert_accepted(&server.post(&heartbeat("sess-slow")).await);
    let restored = upsert(server.take_frame());
    assert_eq!(restored.session_id, "sess-slow", "恢复在线不得换行");
    assert!(restored.online, "写入必须把离线行恢复在线：{restored:?}");

    // 写入已经把这个旗标翻正，所以紧接着的扫描不该再发一帧重复的「恢复在线」。
    server.scan(Instant::now()).await;
    server.assert_no_frame();
    let online = server.snapshot().await;
    assert_eq!(online.len(), 1);
    assert!(online[0].online, "恢复在线后快照如实说「在线」：{online:?}");

    // 显式注销才是行的终点。
    assert_accepted(&server.post(&unregister("sess-slow")).await);
    let Observation::SessionRemove { session_id } = server.take_frame() else {
        panic!("unregister 应发布一帧 remove");
    };
    assert_eq!(session_id, "sess-slow");
    assert!(server.snapshot().await.is_empty());
    server.scan_past_the_window().await;
    server.assert_no_frame();
}

//! 后端权威的关机倒计时（契约 P21、P23、P31、P32、P33、P37、P40、P41、P43；ADR 0005）。
//!
//! 关机安排由**后端进程**持有：手动确认后在这里创建唯一一个全局倒计时，保存安排标识与截止时间，
//! 到期由后端自己调用既有 [`PowerOff`] 能力；前端只展示与提交意图，不拥有到期执行权。
//!
//! 两条裁定规则只在这里、只在一把锁内发生：
//!
//! - **重复发起沿用唯一安排**：已有活动倒计时时再次提交开始意图只是读回当前安排，不创建第二个安排、
//!   不重置也不延长截止时间，更不会因此多调用一次实际关机。
//! - **取消与到期只有一个胜者**：取消在关机执行开始之前生效就阻止本次实际关机；执行已经开始
//!   （阶段推进到 [`ShutdownPhase::Executing`]）就不再允许取消，前端也就不能显示取消成功。
//!
//! **完成后关机监听的集合与自动触发**也在这里、在同一把锁内裁定（契约 P7–P22、P25–P31）：
//!
//! - 只有**当前**在线且工作中的未监听会话可新增监听；设置提交的是明确目标值，不盲目翻转。
//! - 会话状态可能变化时（写入事件之后、会话维护跑过一遍之后）按最新快照重判：**登记已不存在**的成员
//!   被移出，其余成员**当前**是否都在线并等待用户输入决定要不要形成安排——不保存「曾经结束过一次」
//!   的资格。仅仅离线（登记仍在保留期内）的成员保留在集合里并阻塞自动触发。
//! - 集合非空且全部成员当前都在线并等待用户输入时形成唯一安排并**消耗整组监听**；移出最后一个成员
//!   只清空集合。
//! - 已有活动安排时会话重新工作、离线或注销都不撤销它，倒计时期间也不接受新增监听。
//!
//! 最近一次关机结果保留到新安排替代或进程重启：取消、指令已发出与实际失败都是终局阶段，仍然可读；
//! 新的一次开始意图把结果换成新安排，进程重启则把一切都清空（本模块不落盘）。
//!
//! 时间只有两种表示：对外是服务端时钟的 Unix 毫秒（供前端对齐自己的时钟偏差），对内是进程单调时钟的
//! [`Instant`]（到期判定只按它推进，前端计时不参与）。

pub mod power_off;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::Mutex;

use crate::sessions::observation::{Hub, Observation};
use crate::sessions::{SessionView, SessionsState, StateValue};
use crate::shutdown::power_off::{PowerOff, PowerOffFailure, ShutdownOutcome};

/// 关机安排的阶段：调用者可据此区分「没有安排」「还在倒计时」「执行已开始」与三种终局。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShutdownPhase {
    /// 没有活动倒计时，也没有可读的最近结果。
    None,
    /// 倒计时中：截止时间之前仍可取消。
    CountingDown,
    /// 执行已开始：到期已经裁定，实际关机调用已经在路上，不能再取消。
    Executing,
    /// 本次安排已被取消：越过原截止时间也不会发出关机调用。
    Cancelled,
    /// 关机指令已发出。**不是**已经观察到电脑断电。
    Issued,
    /// 实际关机失败：只调用过一次，不自动重试。
    Failed,
}

/// 浏览器可观察的完整关机状态。
///
/// 字段顺序即下发顺序：当前监听集合、安排标识、阶段、截止时间、服务端时间。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownView {
    /// 当前完成后关机监听集合；没有监听时就是权威的空集合。
    pub monitored_session_ids: Vec<String>,
    /// 相关安排标识；从未有过安排时省略。它只用于观察与区分结果，不用于取消目标校验。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrangement_id: Option<String>,
    /// 阶段。
    pub phase: ShutdownPhase,
    /// 截止时间：服务端时钟的 Unix 毫秒；只有倒计时中与执行中可观察。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
    /// 服务端当前时间（Unix 毫秒）：前端据此对齐自己与后端的时钟偏差。
    pub server_time: u64,
}

/// 全局取消的结果类别：公开、机器可读、互相区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CancelOutcome {
    /// 取消已生效：清空监听集合，并撤销了当前尚可取消的倒计时（本来就没有倒计时时同样成立）。
    Cancelled,
    /// 关机执行已经开始：这次取消不会生效。
    NotCancellable,
}

/// 一次监听命令被明确拒绝的原因：公开、机器可读、互相区分，且都不是「没有回应」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorRefusal {
    /// 该会话尚未注册：监听只针对当前存在的会话。
    UnknownSession,
    /// 该会话当前不满足新增监听的条件（不在线，或不是工作中）。
    NotEligible,
    /// 已有活动倒计时：期间不接受新增监听。
    CountdownActive,
}

/// 关机安排：唯一一个活动倒计时。
struct Arrangement {
    /// 安排标识；用于观察与区分结果。
    id: String,
    /// 到期唤醒用的单调时刻。
    deadline: Instant,
    /// 对外可观察的截止时间：创建那一刻算出的服务端时钟 Unix 毫秒，此后**不再变化**。
    ///
    /// 观察值不每次重算：重复发起要沿用同一个截止时间，任何毫秒级的重算漂移都会让调用者看到不同的值。
    deadline_epoch_ms: u64,
    /// 活动阶段：倒计时中或执行已开始。
    phase: ActivePhase,
}

/// 活动安排的阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivePhase {
    /// 倒计时中，仍可取消。
    CountingDown,
    /// 执行已开始，不再可取消。
    Executing,
}

/// 最近一次关机安排的终局；保留到新安排替代或进程重启。
struct Terminal {
    /// 终局阶段：取消已生效、指令已发出或实际失败。
    phase: ShutdownPhase,
    /// 该终局属于哪一个安排。
    arrangement_id: String,
}

/// 一把锁保护的全部状态。
struct Inner {
    /// 当前活动安排；`None` 表示没有活动倒计时。
    active: Option<Arrangement>,
    /// 最近一次终局；被新安排替代或进程重启清除。
    terminal: Option<Terminal>,
    /// 完成后关机监听集合；本票不实现监听选择，全局取消仍然会清空它。
    monitored: BTreeSet<String>,
    /// 安排序号：生成可观察的安排标识。
    next_sequence: u64,
}

/// 后端持有的关机倒计时与完成后关机监听集合。
pub struct ShutdownState {
    inner: Mutex<Inner>,
    /// 会话表：监听资格与自动触发条件都按它的**当前**快照裁定，不缓存资格。
    sessions: Arc<SessionsState>,
    hub: Arc<Hub>,
    power_off: Arc<dyn PowerOff>,
    /// 倒计时时长：启动期配置读入一次，手动与完成后关机共用。
    countdown: Duration,
}

impl ShutdownState {
    /// 建立空状态：没有安排、没有结果、没有监听。
    ///
    /// 倒计时时长来自启动期配置（[`crate::config::ShutdownConfig`]），此后不再重新读取——没有热加载。
    /// 观察流的发布端来自会话表：会话与关机状态合用同一条读连接（[`crate::sessions::observation`]）。
    pub fn new(
        sessions: Arc<SessionsState>,
        power_off: Arc<dyn PowerOff>,
        countdown: Duration,
    ) -> Arc<Self> {
        let hub = sessions.observation();
        Arc::new(Self {
            inner: Mutex::new(Inner {
                active: None,
                terminal: None,
                monitored: BTreeSet::new(),
                next_sequence: 0,
            }),
            sessions,
            hub,
            power_off,
            countdown,
        })
    }

    /// 提交一次手动开始意图。
    ///
    /// 已有活动倒计时时**沿用**它：返回当前安排，不新建、不重置、不延长截止时间。没有活动倒计时时
    /// 创建唯一的新安排（同时**消耗整组监听**），并由后端自己负责到期执行；新安排同时替代最近结果。
    ///
    /// 返回安排创建之后的完整关机状态：接受了开始意图**不等于**关机指令已经发出。
    pub async fn start_manual(self: &Arc<Self>) -> ShutdownView {
        let (view, created) = {
            let mut inner = self.inner.lock().await;
            let server_time = epoch_millis();
            match inner.active {
                Some(_) => (view_locked(&inner, server_time), None),
                None => {
                    let (created, view) =
                        Self::arrange_locked(&mut inner, self.countdown, server_time);
                    // 与状态变化同一把锁内发布：帧顺序与可见变化的发生顺序一致。
                    self.hub.publish(Observation::Shutdown(view.clone()));
                    (view, Some(created))
                }
            }
        };

        // 到期任务在锁外启动：构造上不持有状态锁，也不阻塞这次请求。
        if let Some(created) = created {
            self.spawn_expiry(created);
        }
        view
    }

    /// 设置或撤销某个会话的完成后关机监听：提交的是**明确目标值**，不按当前状态翻转。
    ///
    /// 资格按命令到达时的**当前**会话快照裁定（P7、P44）：未注册 → [`MonitorRefusal::UnknownSession`]；
    /// 不在线或不是工作中 → [`MonitorRefusal::NotEligible`]；已有活动倒计时 →
    /// [`MonitorRefusal::CountdownActive`]。已经监听的会话再次提交选中值只是重申（不反向翻转、也不重置
    /// 任何安排）；提交未选中值表示撤销，撤销未监听的会话是无副作用的重申。
    ///
    /// 新增会在暂存之后**核对一次**：命令处理期间登记可能被清除或注销，这时集合里不能留下不存在的身份
    /// （P40）。核对失败就撤销这次尚未发布的暂存改动，并按当前事实给出拒绝——因此没有任何观察者
    /// 看到过幽灵成员，也不需要等下一遍维护修复。
    ///
    /// 集合变化后在同一把锁内立即重判：撤销或新增都可能让非空集合立即形成安排（P30、P31）。
    pub async fn set_monitoring(
        self: &Arc<Self>,
        session_id: &str,
        selected: bool,
    ) -> Result<ShutdownView, MonitorRefusal> {
        // 资格与状态都按「命令到达时」的当前快照裁定：页面曾经看到什么不参与判定。
        let views = self.sessions.snapshot().await;

        // 这次命令的集合改动；此时还没有发布任何帧，因此失败时可以原样撤销。
        let added = {
            let mut inner = self.inner.lock().await;
            let server_time = epoch_millis();
            let Some(row) = views
                .iter()
                .find(|session| session.session_id == session_id)
            else {
                return Err(MonitorRefusal::UnknownSession);
            };

            if selected {
                if inner.monitored.contains(session_id) {
                    // 重申同一个目标值：集合不变，也不重置任何安排。
                    return Ok(view_locked(&inner, server_time));
                }
                if inner.active.is_some() {
                    return Err(MonitorRefusal::CountdownActive);
                }
                if !row.online || row.state != Some(StateValue::Working) {
                    return Err(MonitorRefusal::NotEligible);
                }
                inner.monitored.insert(session_id.to_string());
                true
            } else if inner.monitored.remove(session_id) {
                false
            } else {
                // 撤销一个没有在监听的会话：无副作用的重申。
                return Ok(view_locked(&inner, server_time));
            }
        };

        // 新增之后核对一次：集合里不能留下在这次命令处理期间已经不存在的登记。
        let views = if added {
            let verified = self.sessions.snapshot().await;
            match verified
                .iter()
                .find(|session| session.session_id == session_id)
            {
                Some(row) if row.online && row.state == Some(StateValue::Working) => verified,
                Some(_) => {
                    self.discard_pending_addition(session_id).await;
                    return Err(MonitorRefusal::NotEligible);
                }
                None => {
                    self.discard_pending_addition(session_id).await;
                    return Err(MonitorRefusal::UnknownSession);
                }
            }
        } else {
            views
        };

        let (view, created) = {
            let mut inner = self.inner.lock().await;
            let server_time = epoch_millis();
            // 集合变了：同一把锁内重判，移出与自动触发都不会漏。
            let (judged, created) = self.judge_locked(&mut inner, &views, server_time);
            // 集合变化**本身**也是一次可见变化：重判没有额外变化时，仍要下发当前集合，
            // 否则其他前端看不到这次新增或撤销。
            let view = judged.unwrap_or_else(|| view_locked(&inner, server_time));
            self.hub.publish(Observation::Shutdown(view.clone()));
            (view, created)
        };

        if let Some(created) = created {
            self.spawn_expiry(created);
        }
        Ok(view)
    }

    /// 撤销一次还没有发布的暂存新增：登记在命令处理期间消失了，集合里不留幽灵成员。
    async fn discard_pending_addition(&self, session_id: &str) {
        let mut inner = self.inner.lock().await;
        inner.monitored.remove(session_id);
    }

    /// 会话状态可能已经变化：按最新快照重判一次监听集合。
    ///
    /// 写入路径在每次被接受的会话事件之后调用它，会话维护（过期、清除）在每遍之后调用它；因此
    /// 「没有事件的那些变化」（上报过期、登记被清除）也会被发现。没有监听成员时什么也不用看——
    /// 空集合既不触发安排，也没有可移出的成员。
    pub async fn sessions_changed(self: &Arc<Self>) -> Option<ShutdownView> {
        {
            let inner = self.inner.lock().await;
            if inner.monitored.is_empty() {
                return None;
            }
        }

        let views = self.sessions.snapshot().await;
        let (published, created) = {
            let mut inner = self.inner.lock().await;
            let (published, created) = self.judge_locked(&mut inner, &views, epoch_millis());
            if let Some(view) = &published {
                self.hub.publish(Observation::Shutdown(view.clone()));
            }
            (published, created)
        };
        if let Some(created) = created {
            self.spawn_expiry(created);
        }
        published
    }

    /// 在锁内按给定快照重判：移出登记已不存在的成员，条件满足就形成唯一安排。
    ///
    /// 返回（要下发的完整关机状态, 新建的安排标识）；第一项为 `None` 表示没有可见变化。
    fn judge_locked(
        &self,
        inner: &mut Inner,
        views: &[SessionView],
        server_time: u64,
    ) -> (Option<ShutdownView>, Option<String>) {
        // 已有活动安排：它已经由后端持有并到期执行，会话变化不撤销它（P30）。
        if inner.active.is_some() {
            return (None, None);
        }

        // 移出**登记已不存在**的成员：登记被清除、明确注销或用户撤销监听之后，集合里不留它。
        // 仅仅离线（上报过期但登记仍在保留期内）的成员保留在集合里，它阻塞自动触发而不是被移出。
        let before = inner.monitored.len();
        inner
            .monitored
            .retain(|id| views.iter().any(|session| &session.session_id == id));
        let removed = inner.monitored.len() != before;

        // 自动触发条件：集合**非空**，且全部成员**当前都在线并等待用户输入**。
        // 离线成员不满足条件（哪怕它最后上报的是等待用户输入）；成员的历史状态不参与判定。
        let all_ready = !inner.monitored.is_empty()
            && inner.monitored.iter().all(|id| {
                views.iter().any(|session| {
                    &session.session_id == id
                        && session.online
                        && session.state == Some(StateValue::AwaitingInput)
                })
            });

        if !all_ready {
            // 空集合（含「移出最后一个成员」）不形成安排；成员离线或恢复工作也只是不满足条件，
            // 不保存「曾经结束过一次」的资格。
            return match removed {
                true => (Some(view_locked(inner, server_time)), None),
                false => (None, None),
            };
        }

        // 条件满足：形成唯一安排，并消耗整组监听。
        let (created, view) = Self::arrange_locked(inner, self.countdown, server_time);
        (Some(view), Some(created))
    }

    /// 建立唯一安排：消耗整组监听、替代最近结果，并返回（安排标识, 此刻的完整状态）。
    ///
    /// 调用者必须持有状态锁，并负责在锁内发布返回的状态、在锁外启动到期任务。
    fn arrange_locked(
        inner: &mut Inner,
        countdown: Duration,
        server_time: u64,
    ) -> (String, ShutdownView) {
        // 开始倒计时即消耗整组监听：取消之后需要用户重新选择，重复等待通知不会自动重启。
        inner.monitored.clear();
        let id = format!("shutdown-{}", inner.next_sequence);
        inner.next_sequence += 1;
        inner.active = Some(Arrangement {
            id: id.clone(),
            deadline: Instant::now() + countdown,
            // 可观察的截止时间在创建时定死：它就是「服务端此刻 + 配置的时长」。
            deadline_epoch_ms: server_time + countdown.as_millis() as u64,
            phase: ActivePhase::CountingDown,
        });
        // 新安排替代旧结果：最近结果不再可读。
        inner.terminal = None;
        let view = view_locked(inner, server_time);
        (id, view)
    }

    /// 启动到期任务：在锁外调用，因此构造上不持有状态锁，也不阻塞调用方。
    fn spawn_expiry(self: &Arc<Self>, arrangement_id: String) {
        let state = Arc::clone(self);
        tokio::spawn(async move { state.expire(arrangement_id).await });
    }

    /// 全局取消：原子清空当前监听集合，并撤销当前**尚可取消**的倒计时。
    ///
    /// 取消作用于收到请求时的当前安排，不按调用者看到的安排标识校验——这是已批准的取舍。
    /// 执行已经开始时不再允许取消（返回 [`CancelOutcome::NotCancellable`]），也不会中断那次执行。
    /// 没有可取消的倒计时时全局清空仍然成立：清空监听本身就是取消的意图。
    pub async fn cancel(&self) -> CancelOutcome {
        let mut inner = self.inner.lock().await;
        let outcome = match inner.active.as_ref().map(|active| active.phase) {
            Some(ActivePhase::Executing) => CancelOutcome::NotCancellable,
            Some(ActivePhase::CountingDown) => {
                let cancelled = inner.active.take().expect("上面刚确认存在活动安排");
                inner.terminal = Some(Terminal {
                    phase: ShutdownPhase::Cancelled,
                    arrangement_id: cancelled.id,
                });
                CancelOutcome::Cancelled
            }
            // 没有活动倒计时：没有可撤销的执行，全局清空照常成立。
            None => CancelOutcome::Cancelled,
        };

        // 全局清空：不逐个移出并重新判定，因此不会中途触发关机。
        let had_monitored = !inner.monitored.is_empty();
        inner.monitored.clear();

        if outcome == CancelOutcome::Cancelled && (had_monitored || inner.terminal.is_some()) {
            let view = view_locked(&inner, epoch_millis());
            self.hub.publish(Observation::Shutdown(view));
        }
        outcome
    }

    /// 到期执行：期限到点后由后端自己裁定并调用既有实际关机能力。
    ///
    /// 先等自己的截止时间，再在锁内把阶段推进到「执行已开始」：推进之后取消不再可能成功，因此
    /// 同一个安排不可能既取消成功又发出调用。调用只发生一次，失败也不重试。
    async fn expire(self: Arc<Self>, arrangement_id: String) {
        let deadline = {
            let inner = self.inner.lock().await;
            match inner.active.as_ref() {
                Some(active) if active.id == arrangement_id => active.deadline,
                _ => return,
            }
        };
        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;

        let claimed = {
            let mut inner = self.inner.lock().await;
            let claimed = match inner.active.as_mut() {
                Some(active)
                    if active.id == arrangement_id && active.phase == ActivePhase::CountingDown =>
                {
                    active.phase = ActivePhase::Executing;
                    true
                }
                _ => false,
            };
            if claimed {
                let view = view_locked(&inner, epoch_millis());
                self.hub.publish(Observation::Shutdown(view));
            }
            claimed
        };
        if !claimed {
            // 取消在裁定之前生效：本次安排不会发出任何关机调用。
            return;
        }

        // 实际关机是同步的系统调用：放到阻塞线程池上执行，异步运行时不因它停住——观察流、取消裁定
        // 与其他请求在这段时间里照常被服务。动作只调用一次，失败也不重试。
        let power_off = Arc::clone(&self.power_off);
        let phase = match tokio::task::spawn_blocking(move || power_off.power_off()).await {
            Ok(Ok(())) => {
                crate::shutdown::power_off::report(&ShutdownOutcome::Invoked);
                ShutdownPhase::Issued
            }
            Ok(Err(failure)) => {
                crate::shutdown::power_off::report(&ShutdownOutcome::Failed(failure));
                ShutdownPhase::Failed
            }
            // 执行任务没有正常结束：结果无法确认，按失败处理，原始原因只进诊断输出。
            Err(error) => {
                crate::shutdown::power_off::report(&ShutdownOutcome::Failed(PowerOffFailure::Internal(
                    format!("关机动作任务没有正常结束：{error}"),
                )));
                ShutdownPhase::Failed
            }
        };

        let mut inner = self.inner.lock().await;
        if inner
            .active
            .as_ref()
            .is_some_and(|active| active.id == arrangement_id)
        {
            inner.active = None;
            inner.terminal = Some(Terminal {
                phase,
                arrangement_id,
            });
            let view = view_locked(&inner, epoch_millis());
            self.hub.publish(Observation::Shutdown(view));
        }
    }

    /// 当前完整关机状态（与观察流下发的是同一份内容）。
    pub async fn snapshot(&self) -> ShutdownView {
        let inner = self.inner.lock().await;
        view_locked(&inner, epoch_millis())
    }
}

/// 取出当前完整关机状态：活动安排优先于终局，两者都没有时就是「没有安排」。
fn view_locked(inner: &Inner, server_time: u64) -> ShutdownView {
    let (phase, arrangement_id, deadline) = match &inner.active {
        Some(active) => (
            match active.phase {
                ActivePhase::CountingDown => ShutdownPhase::CountingDown,
                ActivePhase::Executing => ShutdownPhase::Executing,
            },
            Some(active.id.clone()),
            // 截止时间取创建时定死的值：重复发起与多次读取都必须看到同一个值。
            Some(active.deadline_epoch_ms),
        ),
        None => match &inner.terminal {
            Some(terminal) => (terminal.phase, Some(terminal.arrangement_id.clone()), None),
            None => (ShutdownPhase::None, None, None),
        },
    };
    ShutdownView {
        monitored_session_ids: inner.monitored.iter().cloned().collect(),
        arrangement_id,
        phase,
        deadline,
        server_time,
    }
}

/// 服务端当前时间（Unix 毫秒）。
fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

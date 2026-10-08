//! 单调时间来源：生产路径读进程单调时钟，验收用它把时刻精确推进到窗口边界。
//!
//! 会话登记的生命周期只有一处时间依据（最后一次被接受的写入），因此时间必须只从一个地方取：
//! [`SessionsState`](crate::sessions::SessionsState) 只经本模块的 [`Clock`] 取「现在」，
//! 快照（读路径）、写入与维护扫描共用同一份时刻。这样受控时刻能驱动**真正的读取路径**——
//! 验收里 t=10 的快照说在线、t=11 的快照说离线，都是页面会收到的同一份数据，而不是私有字段。
//!
//! 这是内部组织，不是新的外部 interface：交付运行时用的是 [`SystemClock`]，[`ManualClock`]
//! 只用于把阈值边界变成精确时刻的验收。

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 单调时间来源。
pub trait Clock: Send + Sync + 'static {
    /// 当前单调时刻。
    fn now(&self) -> Instant;
}

/// 生产用时钟：进程单调时钟。
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// 受控时刻：以构造那一刻为 t=0，只有 [`ManualClock::advance`] 才推进它。
///
/// 同一段「虚拟时刻」内的所有读写看到的是同一个 [`Instant`]，因此窗口边界是精确的：
/// 窗口内推进 10 秒就是恰好 10 秒，不掺入真实调度抖动。
#[derive(Debug)]
pub struct ManualClock {
    /// t=0 对应的真实单调时刻。
    base: Instant,
    /// 已推进的时长。
    elapsed: Mutex<Duration>,
}

impl ManualClock {
    /// 从此刻起算的受控时钟；未推进过时就是 t=0。
    pub fn new() -> Self {
        Self {
            base: Instant::now(),
            elapsed: Mutex::new(Duration::ZERO),
        }
    }

    /// 把受控时刻向前推进 `by`。
    pub fn advance(&self, by: Duration) {
        let mut elapsed = self.elapsed.lock().expect("受控时刻的锁不会被毒化");
        *elapsed += by;
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.base + *self.elapsed.lock().expect("受控时刻的锁不会被毒化")
    }
}

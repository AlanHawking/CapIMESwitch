//! CapsLock 长短按状态机。
//!
//! 纯逻辑,不依赖 Win32,可通过时间线驱动单元测试:
//! - 按下后 500ms 内松开 → 短按 → `SwitchIme`
//! - 按下且保持到定时器到期(≥500ms)→ 长按 → `ToggleCapsLock`
//! - 长按已触发后松开 → 无动作
//! - 按住期间按下其他键 → 取消本次动作,松开无动作

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 无动作
    None,
    /// 短按:切换输入法
    SwitchIme,
    /// 长按:切换大小写
    ToggleCapsLock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// 空闲
    Idle,
    /// CapsLock 已按下,等待 500ms 判定
    Held,
    /// 长按已触发,等待松开
    LongFired,
    /// 按住期间按了其他键,本次已取消
    Cancelled,
}

pub struct CapsWatcher {
    state: State,
}

impl Default for CapsWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl CapsWatcher {
    pub const fn new() -> Self {
        Self { state: State::Idle }
    }

    /// CapsLock 按下(非注入事件)。
    /// 返回 true 表示本次按下进入 Held 计时,应启动定时器。
    pub fn on_caps_down(&mut self) -> bool {
        match self.state {
            State::Idle => {
                self.state = State::Held;
                true
            }
            // 自动重复的 keydown 或其他状态下再次按下:忽略,不重置计时
            _ => false,
        }
    }

    /// CapsLock 松开。
    pub fn on_caps_up(&mut self) -> Action {
        match self.state {
            State::Held => {
                self.state = State::Idle;
                Action::SwitchIme
            }
            State::LongFired | State::Cancelled => {
                self.state = State::Idle;
                Action::None
            }
            State::Idle => Action::None,
        }
    }

    /// 定时器到期(距按下已 ≥ 长按阈值)。
    pub fn on_timer(&mut self) -> Action {
        match self.state {
            State::Held => {
                self.state = State::LongFired;
                Action::ToggleCapsLock
            }
            _ => Action::None,
        }
    }

    /// 按住 CapsLock 期间按下了其他键:取消本次待定动作。
    /// 返回 true 表示发生了取消(应停掉定时器)。
    pub fn on_other_key(&mut self) -> bool {
        if self.state == State::Held {
            self.state = State::Cancelled;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_within_threshold_switches_ime() {
        let mut w = CapsWatcher::new();
        assert!(w.on_caps_down(), "首次按下应开始计时");
        assert_eq!(w.on_caps_up(), Action::SwitchIme);
        assert_eq!(w.on_caps_up(), Action::None, "松开后再松开应无动作");
    }

    #[test]
    fn hold_past_threshold_toggles_caps() {
        let mut w = CapsWatcher::new();
        assert!(w.on_caps_down());
        assert_eq!(w.on_timer(), Action::ToggleCapsLock, "定时器到期判定长按");
        // 定时器继续 tick 不应二次触发
        assert_eq!(w.on_timer(), Action::None);
        assert_eq!(w.on_caps_up(), Action::None, "长按后松开无动作");
    }

    #[test]
    fn auto_repeat_down_does_not_restart_timer() {
        let mut w = CapsWatcher::new();
        assert!(w.on_caps_down());
        assert!(!w.on_caps_down(), "重复 keydown 不应重置计时");
        assert_eq!(w.on_timer(), Action::ToggleCapsLock);
    }

    #[test]
    fn other_key_cancels_pending_action() {
        let mut w = CapsWatcher::new();
        w.on_caps_down();
        assert!(w.on_other_key(), "按住期间按其他键应取消");
        assert_eq!(w.on_timer(), Action::None, "取消后定时器不应触发");
        assert_eq!(w.on_caps_up(), Action::None, "取消后松开不应短按");
    }

    #[test]
    fn other_key_while_idle_does_nothing() {
        let mut w = CapsWatcher::new();
        assert!(!w.on_other_key(), "空闲时其他键无影响");
        // 随后正常短按仍生效
        assert!(w.on_caps_down());
        assert_eq!(w.on_caps_up(), Action::SwitchIme);
    }

    #[test]
    fn timer_after_release_is_noop() {
        let mut w = CapsWatcher::new();
        w.on_caps_down();
        w.on_caps_up();
        assert_eq!(w.on_timer(), Action::None, "松开后定时器 tick 无动作");
    }

    #[test]
    fn repeated_cycle_returns_to_idle() {
        let mut w = CapsWatcher::new();
        // 短按
        assert!(w.on_caps_down());
        assert_eq!(w.on_caps_up(), Action::SwitchIme);
        // 长按
        assert!(w.on_caps_down());
        assert_eq!(w.on_timer(), Action::ToggleCapsLock);
        assert_eq!(w.on_caps_up(), Action::None);
        // 又短按
        assert!(w.on_caps_down());
        assert_eq!(w.on_caps_up(), Action::SwitchIme);
    }
}
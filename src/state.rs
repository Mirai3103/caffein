use crate::config::Duration;

pub const ES_CONTINUOUS: u32 = 0x8000_0000;
pub const ES_SYSTEM_REQUIRED: u32 = 0x1;
pub const ES_DISPLAY_REQUIRED: u32 = 0x2;

#[derive(Clone, Copy)]
pub struct State {
    pub active: bool,
    pub keep_display: bool,
    pub duration: Duration,
    deadline_ms: Option<u64>,
}

impl State {
    pub fn new(keep_display: bool, duration: Duration) -> State {
        State {
            active: false,
            keep_display,
            duration,
            deadline_ms: None,
        }
    }

    pub fn activate(&mut self, now_ms: u64) {
        self.active = true;
        self.deadline_ms = self
            .duration
            .minutes()
            .map(|m| now_ms.saturating_add(u64::from(m) * 60_000));
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.deadline_ms = None;
    }

    pub fn toggle(&mut self, now_ms: u64) {
        if self.active {
            self.deactivate();
        } else {
            self.activate(now_ms);
        }
    }

    pub fn set_duration(&mut self, d: Duration, now_ms: u64) {
        self.duration = d;
        if self.active {
            self.activate(now_ms);
        }
    }

    pub fn set_keep_display(&mut self, v: bool) {
        self.keep_display = v;
    }

    pub fn apply_config(&mut self, keep_display: bool, duration: Duration, now_ms: u64) {
        self.set_keep_display(keep_display);
        if self.duration != duration {
            self.set_duration(duration, now_ms);
        }
    }

    pub fn tick(&mut self, now_ms: u64) -> bool {
        if self.active {
            if let Some(deadline) = self.deadline_ms {
                if now_ms >= deadline {
                    self.deactivate();
                    return true;
                }
            }
        }
        false
    }

    pub fn remaining_secs(&self, now_ms: u64) -> Option<u64> {
        if !self.active {
            return None;
        }
        self.deadline_ms
            .map(|deadline| deadline.saturating_sub(now_ms).div_ceil(1000))
    }

    pub fn flags(&self) -> u32 {
        if !self.active {
            ES_CONTINUOUS
        } else if self.keep_display {
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
        } else {
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED
        }
    }

    pub fn needs_timer(&self) -> bool {
        self.active && self.deadline_ms.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Duration;

    #[test]
    fn off_flags() {
        assert_eq!(State::new(true, Duration::Infinite).flags(), ES_CONTINUOUS);
    }

    #[test]
    fn on_flags_with_and_without_display() {
        let mut s = State::new(true, Duration::Infinite);
        s.activate(0);
        assert_eq!(
            s.flags(),
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
        );
        s.set_keep_display(false);
        assert_eq!(s.flags(), ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
    }

    #[test]
    fn timer_expires() {
        let mut s = State::new(false, Duration::M15);
        s.activate(1_000);
        assert!(s.needs_timer());
        assert_eq!(s.remaining_secs(1_000), Some(900));
        assert!(!s.tick(1_000 + 899_999));
        assert!(s.tick(1_000 + 900_000));
        assert!(!s.active && !s.needs_timer());
    }

    #[test]
    fn infinite_never_expires() {
        let mut s = State::new(false, Duration::Infinite);
        s.activate(0);
        assert!(!s.needs_timer());
        assert!(!s.tick(u64::MAX / 2));
        assert!(s.active);
        assert_eq!(s.remaining_secs(5), None);
    }

    #[test]
    fn change_duration_while_active_restarts() {
        let mut s = State::new(false, Duration::M15);
        s.activate(0);
        s.set_duration(Duration::H1, 600_000);
        assert_eq!(s.remaining_secs(600_000), Some(3600));
    }

    #[test]
    fn toggle_roundtrip() {
        let mut s = State::new(false, Duration::M30);
        s.toggle(0);
        assert!(s.active);
        s.toggle(1);
        assert!(!s.active);
        assert_eq!(s.remaining_secs(1), None);
    }

    #[test]
    fn save_unchanged_duration_preserves_countdown() {
        let mut s = State::new(true, Duration::H2);
        s.activate(0);
        s.apply_config(false, Duration::H2, 6_600_000);
        assert!(!s.keep_display);
        assert_eq!(s.remaining_secs(6_600_000), Some(600));
    }
}

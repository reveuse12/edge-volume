#[derive(Default)]
pub struct AudioGuard {
    pub device: u32,
    pub available: bool,
}
impl AudioGuard {
    // Caller cancels the current touch sequence whenever the output changes or
    // becomes unavailable. User enabled intent is deliberately kept elsewhere.
    pub fn update(&mut self, device: u32, available: bool) -> bool {
        let cancel = self.device != device || self.available != available;
        self.device = device;
        self.available = available;
        cancel
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_changes_and_failure_recovery_cancel_old_gestures() {
        let mut g = AudioGuard::default();
        assert!(g.update(1, true));
        assert!(!g.update(1, true));
        assert!(g.update(2, true));
        assert!(g.update(2, false));
        assert!(!g.update(2, false));
        assert!(g.update(3, true));
    }
}

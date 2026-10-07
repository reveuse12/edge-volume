use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub edge: String,
    pub width: f32,
    pub sensitivity: f32,
    pub inverted: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            edge: "right".into(),
            width: 0.08,
            sensitivity: 1.0,
            inverted: false,
        }
    }
}
impl Settings {
    pub fn valid(&self) -> bool {
        ["left", "right", "both"].contains(&self.edge.as_str())
            && self.width.is_finite()
            && (0.03..=0.20).contains(&self.width)
            && self.sensitivity.is_finite()
            && (0.25..=2.0).contains(&self.sensitivity)
    }
}
#[derive(Default)]
pub struct Gesture {
    contact: Option<i32>,
    previous: f32,
    origin: f32,
    active: bool,
    rejected: bool,
}
impl Gesture {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn feed(&mut self, s: &Settings, count: i32, id: i32, x: f32, y: f32) -> Option<f32> {
        if count == 0 {
            self.reset();
            return None;
        }
        if !s.enabled || count != 1 || !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            self.rejected = true;
            return None;
        }
        let inside =
            (s.edge != "left" && x >= 1.0 - s.width) || (s.edge != "right" && x <= s.width);
        if self.contact.is_none() {
            self.contact = Some(id);
            self.origin = y;
            self.previous = y;
            self.rejected = !inside;
            return None;
        }
        if self.rejected {
            return None;
        }
        if self.contact != Some(id) || !inside {
            self.rejected = true;
            return None;
        }
        let delta = y - self.previous;
        self.previous = y;
        if delta.abs() > 0.15 {
            self.rejected = true;
            return None;
        }
        if !self.active {
            if (y - self.origin).abs() < 0.025 {
                return None;
            }
            self.active = true;
        }
        Some(delta.clamp(-0.025, 0.025) * s.sensitivity * if s.inverted { -1.0 } else { 1.0 })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> Settings {
        Settings {
            enabled: true,
            ..Settings::default()
        }
    }
    #[test]
    fn starts_at_edge_and_activates_after_deadzone() {
        let mut g = Gesture::default();
        let s = settings();
        assert_eq!(g.feed(&s, 1, 1, 0.98, 0.3), None);
        assert_eq!(g.feed(&s, 1, 1, 0.98, 0.31), None);
        assert!(g.feed(&s, 1, 1, 0.98, 0.34).unwrap() > 0.0);
    }
    #[test]
    fn cannot_enter_from_middle() {
        let mut g = Gesture::default();
        let s = settings();
        g.feed(&s, 1, 1, 0.5, 0.3);
        assert_eq!(g.feed(&s, 1, 1, 0.99, 0.4), None);
    }
    #[test]
    fn second_finger_rejects_until_lift() {
        let mut g = Gesture::default();
        let s = settings();
        g.feed(&s, 1, 1, 0.99, 0.3);
        g.feed(&s, 2, 1, 0.99, 0.4);
        assert_eq!(g.feed(&s, 1, 1, 0.99, 0.5), None);
        g.feed(&s, 0, 0, 0.0, 0.0);
        g.feed(&s, 1, 1, 0.99, 0.3);
        assert!(g.feed(&s, 1, 1, 0.99, 0.35).is_some());
    }
    #[test]
    fn leaving_zone_rejects() {
        let mut g = Gesture::default();
        let s = settings();
        g.feed(&s, 1, 1, 0.99, 0.3);
        g.feed(&s, 1, 1, 0.5, 0.35);
        assert_eq!(g.feed(&s, 1, 1, 0.99, 0.4), None);
    }
    #[test]
    fn inversion_and_jump_guard() {
        let mut g = Gesture::default();
        let mut s = settings();
        s.inverted = true;
        g.feed(&s, 1, 1, 0.99, 0.3);
        assert!(g.feed(&s, 1, 1, 0.99, 0.35).unwrap() < 0.0);
        assert_eq!(g.feed(&s, 1, 1, 0.99, 0.9), None);
    }
}

use serde::{Deserialize, Serialize};
use std::time::Duration;

fn default_modifier() -> bool {
    false
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub edge: String,
    pub width: f32,
    pub sensitivity: f32,
    pub inverted: bool,
    #[serde(default = "default_modifier")]
    pub require_modifier: bool,
    #[serde(default)]
    pub excluded_apps: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            edge: "right".into(),
            width: 0.08,
            sensitivity: 1.0,
            inverted: false,
            require_modifier: false,
            excluded_apps: Vec::new(),
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
            && self.excluded_apps.len() <= 32
            && self.excluded_apps.iter().all(|id| {
                !id.is_empty()
                    && id.len() <= 200
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c))
            })
    }
}
pub struct Touch {
    pub count: i32,
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub at: Duration,
    pub modifier: bool,
}
#[derive(Default)]
pub struct Gesture {
    contact: Option<i32>,
    origin_x: f32,
    origin_y: f32,
    previous_y: f32,
    last: Option<Duration>,
    left: bool,
    active: bool,
    rejected: bool,
    rejection: Option<&'static str>,
}
impl Gesture {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn cancel(&mut self) {
        self.reject("Lift all fingers before starting again");
    }
    fn reject(&mut self, reason: &'static str) {
        self.rejected = true;
        self.active = false;
        self.rejection = Some(reason);
    }
    pub fn hint(&self) -> &'static str {
        self.rejection.unwrap_or(if self.active {
            "Adjusting volume"
        } else if self.contact.is_some() {
            "Slide vertically to adjust volume"
        } else {
            "Start one finger inside the selected edge"
        })
    }
    pub fn feed(&mut self, s: &Settings, t: Touch) -> Option<f32> {
        if t.count == 0 {
            self.reset();
            return None;
        }
        let elapsed = self.last.map(|last| t.at.checked_sub(last));
        self.last = Some(t.at);
        // Missing/reordered frames (including sleep) cannot resume an old swipe.
        if elapsed.is_some_and(|d| d.is_none_or(|d| self.active && d > Duration::from_millis(250)))
        {
            self.reject("Input interrupted; lift all fingers and retry");
            return None;
        }
        if !s.enabled
            || t.count != 1
            || (s.require_modifier && !t.modifier)
            || !(0.0..=1.0).contains(&t.x)
            || !(0.0..=1.0).contains(&t.y)
        {
            self.reject(if !s.enabled {
                "Gestures are paused"
            } else if t.count != 1 {
                "Use one finger; lift all fingers and retry"
            } else if s.require_modifier && !t.modifier {
                "Hold Option before starting the swipe"
            } else {
                "Invalid touch sample; lift all fingers and retry"
            });
            return None;
        }
        // Rejection is latched even if no single contact has been assigned yet.
        if self.rejected {
            return None;
        }
        if self.contact.is_none() {
            let left = s.edge != "right" && t.x <= s.width;
            let right = s.edge != "left" && t.x >= 1.0 - s.width;
            if !left && !right {
                self.reject("Start inside the selected edge; lift all fingers and retry");
                return None;
            }
            self.contact = Some(t.id);
            self.left = left;
            self.origin_x = t.x;
            self.origin_y = t.y;
            self.previous_y = t.y;
            return None;
        }
        let inside = if self.left {
            t.x <= s.width
        } else {
            t.x >= 1.0 - s.width
        };
        let dx = t.x - self.origin_x;
        let dy = t.y - self.origin_y;
        let step = t.y - self.previous_y;
        if self.contact != Some(t.id) || !inside || dx.abs() > 0.06 || step.abs() > 0.15 {
            self.reject("Touch left the edge or changed; lift all fingers and retry");
            return None;
        }
        self.previous_y = t.y;
        if !self.active {
            if dy.abs() < 0.03 {
                return None;
            }
            if dx.abs() > dy.abs() * 0.6 {
                self.reject("Swipe vertically; lift all fingers and retry");
                return None;
            }
            self.active = true;
        }
        // Bound changes by elapsed time, not frame frequency. No catch-up jump.
        let seconds = elapsed.flatten().unwrap_or_default().as_secs_f32().min(0.1);
        let limit = 0.6 * seconds;
        Some((step * s.sensitivity).clamp(-limit, limit) * if s.inverted { -1.0 } else { 1.0 })
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
    fn frame(
        g: &mut Gesture,
        s: &Settings,
        n: i32,
        id: i32,
        x: f32,
        y: f32,
        ms: u64,
        key: bool,
    ) -> Option<f32> {
        g.feed(
            s,
            Touch {
                count: n,
                id,
                x,
                y,
                at: Duration::from_millis(ms),
                modifier: key,
            },
        )
    }
    fn arm(g: &mut Gesture, s: &Settings, x: f32) {
        assert_eq!(frame(g, s, 1, 1, x, 0.3, 0, true), None);
        assert_eq!(frame(g, s, 1, 1, x, 0.3, 200, true), None);
    }
    #[test]
    fn intentional_vertical_motion() {
        let s = settings();
        let mut g = Gesture::default();
        arm(&mut g, &s, 0.98);
        assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.31, 216, true), None);
        assert!(frame(&mut g, &s, 1, 1, 0.98, 0.34, 232, true).unwrap() > 0.0);
    }
    #[test]
    fn ordinary_swipe_without_option_never_arms() {
        let s = Settings {
            require_modifier: true,
            ..settings()
        };
        let mut g = Gesture::default();
        for i in 0..15 {
            assert_eq!(
                frame(&mut g, &s, 1, 1, 0.98, 0.3 + i as f32 * 0.02, i * 16, false),
                None
            );
        }
        // Pressing Option after the touch began also cannot arm it.
        assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.6, 240, true), None);
    }
    #[test]
    fn normal_edge_swipe_works_without_option_or_a_hold() {
        let s = settings();
        let mut g = Gesture::default();
        assert!(!s.require_modifier);
        frame(&mut g, &s, 1, 1, 0.98, 0.3, 0, false);
        assert!(frame(&mut g, &s, 1, 1, 0.97, 0.35, 16, false).unwrap() > 0.0);
    }
    #[test]
    fn stationary_contact_without_repeated_frames_can_start_moving() {
        let s = settings();
        let mut g = Gesture::default();
        frame(&mut g, &s, 1, 1, 0.98, 0.3, 0, false);
        assert!(frame(&mut g, &s, 1, 1, 0.98, 0.35, 1000, false).unwrap() > 0.0);
        // A gap during an active adjustment still cancels rather than catching up.
        assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.4, 2000, false), None);
    }
    #[test]
    fn horizontal_swipe_rejects_even_inside_the_strip() {
        let s = settings();
        let mut g = Gesture::default();
        frame(&mut g, &s, 1, 1, 0.98, 0.3, 0, false);
        assert_eq!(frame(&mut g, &s, 1, 1, 0.94, 0.33, 16, false), None);
        assert_eq!(frame(&mut g, &s, 1, 1, 0.94, 0.4, 32, false), None);
    }
    #[test]
    fn left_right_and_both_share_safety_rules() {
        for (edge, x) in [
            ("left", 0.02),
            ("right", 0.98),
            ("both", 0.02),
            ("both", 0.98),
        ] {
            let s = Settings {
                edge: edge.into(),
                ..settings()
            };
            let mut g = Gesture::default();
            arm(&mut g, &s, x);
            assert!(frame(&mut g, &s, 1, 1, x, 0.35, 216, true).is_some());
        }
    }
    #[test]
    fn cannot_enter_from_middle() {
        let s = settings();
        let mut g = Gesture::default();
        frame(&mut g, &s, 1, 1, 0.5, 0.3, 0, true);
        assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.3, 200, true), None);
    }
    #[test]
    fn second_finger_at_start_stays_rejected_until_lift() {
        let s = settings();
        let mut g = Gesture::default();
        frame(&mut g, &s, 2, 1, 0.98, 0.3, 0, true);
        assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.3, 200, true), None);
        assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.35, 216, true), None);
        frame(&mut g, &s, 0, 0, 0.0, 0.0, 240, true);
        arm(&mut g, &s, 0.98);
        assert!(frame(&mut g, &s, 1, 1, 0.98, 0.35, 216, true).is_some());
    }
    #[test]
    fn added_finger_release_or_changed_identity_cancels() {
        for (n, id, key) in [(2, 1, true), (1, 1, false), (1, 2, true)] {
            let s = Settings {
                require_modifier: true,
                ..settings()
            };
            let mut g = Gesture::default();
            arm(&mut g, &s, 0.98);
            assert_eq!(frame(&mut g, &s, n, id, 0.98, 0.35, 216, key), None);
            assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.4, 232, true), None);
        }
    }
    #[test]
    fn cannot_switch_edges_or_move_sideways() {
        for x in [0.5, 0.02, 0.9] {
            let s = Settings {
                edge: "both".into(),
                ..settings()
            };
            let mut g = Gesture::default();
            arm(&mut g, &s, 0.98);
            assert_eq!(frame(&mut g, &s, 1, 1, x, 0.35, 216, true), None);
        }
    }
    #[test]
    fn stale_or_reordered_frames_cancel() {
        for ms in [501, 215] {
            let s = settings();
            let mut g = Gesture::default();
            arm(&mut g, &s, 0.98);
            frame(&mut g, &s, 1, 1, 0.98, 0.34, 216, true);
            assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.35, ms, true), None);
        }
    }
    #[test]
    fn invalid_coordinates_latch_rejection() {
        for x in [f32::NAN, f32::INFINITY, -1.0, 2.0] {
            let s = settings();
            let mut g = Gesture::default();
            frame(&mut g, &s, 1, 1, x, 0.3, 0, true);
            assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.3, 200, true), None);
        }
    }
    #[test]
    fn speed_is_bounded_and_direction_can_reverse() {
        let s = Settings {
            inverted: true,
            sensitivity: 2.0,
            ..settings()
        };
        let mut g = Gesture::default();
        arm(&mut g, &s, 0.98);
        let delta = frame(&mut g, &s, 1, 1, 0.98, 0.4, 216, true).unwrap();
        assert!(delta < 0.0 && delta.abs() <= 0.6 * 0.016 + 0.00001);
        assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.9, 232, true), None);
    }
    #[test]
    fn cancel_requires_lift_and_legacy_settings_migrate_safely() {
        let s: Settings = serde_json::from_str(
            r#"{"enabled":true,"edge":"right","width":0.08,"sensitivity":1,"inverted":false}"#,
        )
        .unwrap();
        assert!(!s.require_modifier && s.excluded_apps.is_empty());
        let mut g = Gesture::default();
        g.cancel();
        assert_eq!(frame(&mut g, &s, 1, 1, 0.98, 0.3, 0, true), None);
    }
}

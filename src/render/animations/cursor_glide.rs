use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CursorGlideState {
    pub prev_x: f32,
    pub cur_x: f32,
    pub target_x: f32,
    pub start_time: Instant,
    pub duration_secs: f32,
    pub is_animating: bool,
    pub width: f32,
    #[allow(dead_code)]
    pub corner_radius: f32,
    pub trail_smear: f32,
    pub current_smear: Option<(f32, f32)>,
}

impl CursorGlideState {
    pub fn new(duration_ms: u64) -> Self {
        Self {
            prev_x: 0.0,
            cur_x: 0.0,
            target_x: 0.0,
            start_time: Instant::now(),
            duration_secs: (duration_ms.max(10) as f32) / 1000.0,
            is_animating: false,
            width: 3.0,
            corner_radius: 1.5,
            trail_smear: 0.85,
            current_smear: None,
        }
    }

    pub fn set_target(&mut self, x: f32) {
        if (self.target_x - x).abs() < 0.2 {
            return;
        }
        self.prev_x = self.cur_x;
        self.target_x = x;
        self.start_time = Instant::now();
        self.is_animating = true;
    }

    #[allow(dead_code)]
    pub fn reset(&mut self, x: f32) {
        self.prev_x = x;
        self.cur_x = x;
        self.target_x = x;
        self.is_animating = false;
        self.current_smear = None;
    }

    pub fn update(&mut self) -> bool {
        if !self.is_animating {
            return false;
        }

        let elapsed = self.start_time.elapsed().as_secs_f32();
        let t = (elapsed / self.duration_secs).clamp(0.0, 1.0);

        if t >= 1.0 || (self.target_x - self.prev_x).abs() < 0.5 {
            self.cur_x = self.target_x;
            self.prev_x = self.target_x;
            self.is_animating = false;
            self.current_smear = None;
            return true;
        }

        let t_lead = ease_leading(t);
        self.cur_x = self.prev_x + (self.target_x - self.prev_x) * t_lead;

        if self.trail_smear > 0.01 && (self.target_x - self.prev_x).abs() >= 1.0 {
            let t_trail = t_lead + (ease_trailing(t) - t_lead) * self.trail_smear;
            let lead_x = self.cur_x;
            let trail_x = self.prev_x + (self.target_x - self.prev_x) * t_trail;
            let left_x = lead_x.min(trail_x);
            let right_x = (lead_x + self.width).max(trail_x + self.width);
            let smear_w = (right_x - left_x).max(self.width);
            self.current_smear = Some((left_x, smear_w));
        } else {
            self.current_smear = None;
        }

        true
    }
}

fn ease_leading(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(4)
}

fn ease_trailing(x: f32) -> f32 {
    x.powf(2.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_glide_moves_toward_target() {
        let mut glide = CursorGlideState::new(150);
        glide.set_target(100.0);
        assert_eq!(glide.cur_x, 0.0);
        assert_eq!(glide.target_x, 100.0);
        assert!(glide.is_animating);

        std::thread::sleep(std::time::Duration::from_millis(20));
        let moving = glide.update();
        assert!(moving);
        assert!(glide.cur_x > 0.0);
        assert!(glide.cur_x <= 100.0);
        assert!(glide.current_smear.is_some());
    }

    #[test]
    fn test_glide_reaches_target_and_stops() {
        let mut glide = CursorGlideState::new(50);
        glide.set_target(100.0);

        std::thread::sleep(std::time::Duration::from_millis(70));
        let final_frame = glide.update();
        assert!(final_frame);
        assert!(!glide.is_animating);
        assert_eq!(glide.cur_x, 100.0);
        assert_eq!(glide.current_smear, None);

        assert!(!glide.update());
    }
}

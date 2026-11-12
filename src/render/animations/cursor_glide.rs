use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CursorGlideState {
    pub current_x: f32,
    pub target_x: f32,
    pub trail_x: f32,
    pub last_frame: Instant,
    pub duration_ms: f32,
}

impl CursorGlideState {
    pub fn new(duration_ms: u64) -> Self {
        Self {
            current_x: 0.0,
            target_x: 0.0,
            trail_x: 0.0,
            last_frame: Instant::now(),
            duration_ms: duration_ms.max(10) as f32,
        }
    }

    pub fn set_target(&mut self, x: f32) {
        self.trail_x = self.current_x;
        self.target_x = x;
    }

    #[allow(dead_code)]
    pub fn reset(&mut self, x: f32) {
        self.current_x = x;
        self.target_x = x;
        self.trail_x = x;
        self.last_frame = Instant::now();
    }

    pub fn update(&mut self) -> bool {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32() * 1000.0;
        self.last_frame = now;

        let diff = self.target_x - self.current_x;
        if diff.abs() < 0.2 {
            self.current_x = self.target_x;
            self.trail_x = self.target_x;
            return false;
        }

        let speed = 1.0 - (-dt / (self.duration_ms * 0.4)).exp();
        self.current_x += diff * speed;

        let trail_diff = self.current_x - self.trail_x;
        self.trail_x += trail_diff * (speed * 0.7);

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_glide_moves_toward_target() {
        let mut glide = CursorGlideState::new(150);
        glide.set_target(100.0);
        assert_eq!(glide.current_x, 0.0);
        assert_eq!(glide.target_x, 100.0);

        std::thread::sleep(std::time::Duration::from_millis(20));
        let moving = glide.update();
        assert!(moving);
        assert!(glide.current_x > 0.0);
        assert!(glide.current_x <= 100.0);
    }
}

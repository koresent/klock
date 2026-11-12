use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ShakeAnimation {
    start_time: Option<Instant>,
    duration_secs: f32,
    amplitude: f32,
    frequency: f32,
}

impl ShakeAnimation {
    pub fn new() -> Self {
        Self {
            start_time: None,
            duration_secs: 0.35,
            amplitude: 14.0,
            frequency: 38.0,
        }
    }

    pub fn trigger(&mut self) {
        self.start_time = Some(Instant::now());
    }

    pub fn is_active(&self) -> bool {
        if let Some(start) = self.start_time {
            start.elapsed().as_secs_f32() < self.duration_secs
        } else {
            false
        }
    }

    pub fn offset_x(&self) -> f32 {
        if let Some(start) = self.start_time {
            let t = start.elapsed().as_secs_f32();
            if t >= self.duration_secs {
                return 0.0;
            }
            let decay = 1.0 - (t / self.duration_secs);
            (t * self.frequency).sin() * self.amplitude * decay * decay
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shake_idle_and_trigger() {
        let mut shake = ShakeAnimation::new();
        assert!(!shake.is_active());
        assert_eq!(shake.offset_x(), 0.0);

        shake.trigger();
        assert!(shake.is_active());
    }
}

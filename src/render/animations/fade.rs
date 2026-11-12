use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadePhase {
    PendingFadeIn,
    FadeIn,
    Visible,
    FadeOut,
    Done,
}

#[derive(Debug, Clone)]
pub struct FadeAnimation {
    pub phase: FadePhase,
    start_time: Instant,
    duration_secs: f32,
}

impl FadeAnimation {
    pub fn new(enabled: bool, duration_ms: u64) -> Self {
        Self {
            phase: if enabled {
                FadePhase::PendingFadeIn
            } else {
                FadePhase::Visible
            },
            start_time: Instant::now(),
            duration_secs: (duration_ms.max(20) as f32) / 1000.0,
        }
    }

    pub fn start_fade_in(&mut self) {
        if self.phase == FadePhase::PendingFadeIn {
            self.phase = FadePhase::FadeIn;
            self.start_time = Instant::now();
        }
    }

    pub fn trigger_fade_out(&mut self) {
        if self.phase == FadePhase::Done {
            return;
        }
        self.phase = FadePhase::FadeOut;
        self.start_time = Instant::now();
    }

    pub fn is_done(&self) -> bool {
        self.phase == FadePhase::Done
    }

    pub fn update(&mut self) -> bool {
        match self.phase {
            FadePhase::PendingFadeIn => false,
            FadePhase::FadeIn => {
                if self.start_time.elapsed().as_secs_f32() >= self.duration_secs {
                    self.phase = FadePhase::Visible;
                }
                true
            }
            FadePhase::FadeOut => {
                if self.start_time.elapsed().as_secs_f32() >= self.duration_secs {
                    self.phase = FadePhase::Done;
                }
                true
            }
            FadePhase::Visible | FadePhase::Done => false,
        }
    }

    pub fn opacity(&self) -> f32 {
        match self.phase {
            FadePhase::PendingFadeIn => 0.0,
            FadePhase::FadeIn => {
                let t =
                    (self.start_time.elapsed().as_secs_f32() / self.duration_secs).clamp(0.0, 1.0);
                1.0 - (1.0 - t).powi(2)
            }
            FadePhase::Visible => 1.0,
            FadePhase::FadeOut => {
                let t =
                    (self.start_time.elapsed().as_secs_f32() / self.duration_secs).clamp(0.0, 1.0);
                (1.0 - t).powi(2)
            }
            FadePhase::Done => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fade_in_and_out_lifecycle() {
        let mut fade = FadeAnimation::new(true, 30);
        assert_eq!(fade.phase, FadePhase::PendingFadeIn);
        assert_eq!(fade.opacity(), 0.0);
        assert!(!fade.update());

        fade.start_fade_in();
        assert_eq!(fade.phase, FadePhase::FadeIn);
        assert!(fade.opacity() <= 1.0);

        std::thread::sleep(std::time::Duration::from_millis(45));
        assert!(fade.update());
        assert_eq!(fade.phase, FadePhase::Visible);
        assert_eq!(fade.opacity(), 1.0);
        assert!(!fade.update());

        fade.trigger_fade_out();
        assert_eq!(fade.phase, FadePhase::FadeOut);
        std::thread::sleep(std::time::Duration::from_millis(45));
        assert!(fade.update());
        assert_eq!(fade.phase, FadePhase::Done);
        assert!(fade.is_done());
    }
}

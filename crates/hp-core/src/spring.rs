//! A damped spring, stepped at a fixed small time step so motion does not depend on the frame
//! rate. Used for the zoom camera (overdamped: it never overshoots).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub tension: f32,
    pub mass: f32,
    pub friction: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpringState {
    pub value: f32,
    pub velocity: f32,
}

/// The zoom camera: smooth, no bounce (Screen-Studio-Effects "focused" profile).
pub const CAMERA: Spring = Spring { tension: 300.0, mass: 6.75, friction: 120.0 };

impl Spring {
    /// Advances `s` towards `target` by `dt_s` seconds (semi-implicit Euler).
    pub fn step(&self, s: &mut SpringState, target: f32, dt_s: f32) {
        let accel = (self.tension * (target - s.value) - self.friction * s.velocity) / self.mass;
        s.velocity += accel * dt_s;
        s.value += s.velocity * dt_s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(from: f32, to: f32, ms: usize) -> Vec<f32> {
        let mut s = SpringState { value: from, velocity: 0.0 };
        (0..ms)
            .map(|_| {
                CAMERA.step(&mut s, to, 0.001);
                s.value
            })
            .collect()
    }

    #[test]
    fn settles_on_the_target_without_overshoot() {
        let v = run(0.0, 1.0, 3000);
        assert!(v.iter().all(|x| *x <= 1.0 + 1e-6), "overshoot: max {}", v.iter().cloned().fold(0.0, f32::max));
        assert!((v.last().unwrap() - 1.0).abs() < 0.01, "ends at {}", v.last().unwrap());
    }

    #[test]
    fn moves_monotonically() {
        let v = run(1.0, 2.0, 2000);
        assert!(v.windows(2).all(|w| w[1] >= w[0] - 1e-6));
        assert!(v[0] > 1.0, "it starts moving at once");
    }

    #[test]
    fn stays_put_at_the_target() {
        assert!(run(0.5, 0.5, 500).iter().all(|x| (*x - 0.5).abs() < 1e-6));
    }
}

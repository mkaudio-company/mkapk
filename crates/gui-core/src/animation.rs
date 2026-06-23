use alloc::boxed::Box;
use alloc::vec::Vec;
use core::time::Duration;

use crate::color::Color;
use crate::transform::{Transform, sin_cos};

const TWO_PI: f32 = 2.0 * core::f32::consts::PI;

#[allow(clippy::manual_clamp)]
fn clamp01(t: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else if t > 1.0 {
        1.0
    } else {
        t
    }
}

/// `exp(x)` approximation using a Taylor series. `core::f32` does not expose
/// transcendental functions under `#![no_std]`, so we keep this local to the
/// animation spring curve.
fn manual_exp(x: f32) -> f32 {
    let mut term = 1.0;
    let mut sum = 1.0;
    let mut n = 1.0;
    while n < 16.0 {
        term *= x / n;
        sum += term;
        n += 1.0;
    }
    sum
}

/// Types that can be linearly interpolated by the animation engine.
pub trait Animatable: Clone {
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

impl Animatable for f32 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = clamp01(t);
        *self + (*other - *self) * t
    }
}

impl Animatable for Color {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = clamp01(t);
        let inv = 1.0 - t;
        let r = self.r as f32 / 255.0 * inv + other.r as f32 / 255.0 * t;
        let g = self.g as f32 / 255.0 * inv + other.g as f32 / 255.0 * t;
        let b = self.b as f32 / 255.0 * inv + other.b as f32 / 255.0 * t;
        let a = self.a as f32 / 255.0 * inv + other.a as f32 / 255.0 * t;
        Color::from_f32(r, g, b, a)
    }
}

impl Animatable for Transform {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = clamp01(t);
        let inv = 1.0 - t;
        Transform {
            m11: self.m11 * inv + other.m11 * t,
            m12: self.m12 * inv + other.m12 * t,
            m21: self.m21 * inv + other.m21 * t,
            m22: self.m22 * inv + other.m22 * t,
            m31: self.m31 * inv + other.m31 * t,
            m32: self.m32 * inv + other.m32 * t,
        }
    }
}

/// Easing curve applied to normalized time `t` in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AnimationCurve {
    #[default]
    Linear,
    EaseInOut,
    Spring,
}

impl AnimationCurve {
    /// Map normalized time `t` to an eased progress value.
    ///
    /// `t` is clamped to `[0, 1]` before easing. The returned value may lie
    /// outside `[0, 1]` for overshooting curves such as [`AnimationCurve::Spring`].
    pub fn ease(&self, t: f32) -> f32 {
        let t = clamp01(t);
        match self {
            AnimationCurve::Linear => t,
            AnimationCurve::EaseInOut => t * t * (3.0 - 2.0 * t),
            AnimationCurve::Spring => {
                const DAMPING: f32 = 6.0;
                const FREQUENCY: f32 = 3.0;
                let omega = TWO_PI * FREQUENCY;
                let (sin, cos) = sin_cos(omega * t);
                // Under-damped spring: 1 - e^(-damping*t) * (cos(omega*t) + damping/omega * sin(omega*t))
                // This form starts at 0, overshoots, and settles at 1.
                let envelope = manual_exp(-DAMPING * t);
                let oscillation = cos + (DAMPING / omega) * sin;
                1.0 - envelope * oscillation
            }
        }
    }
}

/// Lifecycle state of an [`Animation`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AnimationState {
    #[default]
    Idle,
    Running,
    Paused,
    Completed,
    Stopped,
}

/// Result of advancing an animation by one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationTick<T: Animatable> {
    Running(T),
    Completed(T),
}

/// A single animated transition between two values.
pub struct Animation<T: Animatable> {
    from: T,
    to: T,
    duration: Duration,
    elapsed: Duration,
    curve: AnimationCurve,
    state: AnimationState,
    on_complete: Option<Box<dyn FnOnce()>>,
}

impl<T: Animatable> Animation<T> {
    /// Create a new animation from `from` to `to` over `duration`.
    pub fn new(from: T, to: T, duration: Duration) -> Self {
        Self {
            from,
            to,
            duration,
            elapsed: Duration::ZERO,
            curve: AnimationCurve::Linear,
            state: AnimationState::Idle,
            on_complete: None,
        }
    }

    /// Set the easing curve.
    pub fn with_curve(mut self, curve: AnimationCurve) -> Self {
        self.curve = curve;
        self
    }

    /// Register a callback invoked exactly once when the animation completes.
    pub fn on_complete<F: FnOnce() + 'static>(mut self, f: F) -> Self {
        self.on_complete = Some(Box::new(f));
        self
    }

    /// Begin or restart playback from the beginning.
    pub fn start(&mut self) {
        self.elapsed = Duration::ZERO;
        self.state = AnimationState::Running;
    }

    /// Stop playback. The animation will be removed from the controller on the
    /// next tick and will not fire its completion callback.
    pub fn stop(&mut self) {
        self.state = AnimationState::Stopped;
    }

    /// Pause playback. Resume with [`Animation::resume`].
    pub fn pause(&mut self) {
        if self.state == AnimationState::Running {
            self.state = AnimationState::Paused;
        }
    }

    /// Resume a paused animation.
    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            self.state = AnimationState::Running;
        }
    }

    /// Reset elapsed time and start running.
    pub fn restart(&mut self) {
        self.start();
    }

    /// Current lifecycle state.
    pub fn state(&self) -> AnimationState {
        self.state
    }

    /// Elapsed playback time.
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Total animation duration.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Current interpolated value.
    pub fn value(&self) -> T {
        let t = normalized_time(self.elapsed, self.duration);
        self.from.lerp(&self.to, self.curve.ease(t))
    }

    /// Advance the animation by `dt` and report its new state.
    ///
    /// Returns `None` when the animation is idle, paused, stopped, or already
    /// completed.
    pub fn tick(&mut self, dt: Duration) -> Option<AnimationTick<T>> {
        if self.state != AnimationState::Running {
            return None;
        }

        self.elapsed = self.elapsed.saturating_add(dt);

        if self.elapsed >= self.duration || self.duration.is_zero() {
            self.elapsed = self.duration;
            self.state = AnimationState::Completed;
            let value = self.to.clone();
            if let Some(cb) = self.on_complete.take() {
                cb();
            }
            Some(AnimationTick::Completed(value))
        } else {
            Some(AnimationTick::Running(self.value()))
        }
    }
}

fn normalized_time(elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    elapsed.as_secs_f32() / duration.as_secs_f32()
}

/// Unique identifier returned by [`AnimationController::start`].
pub type AnimationId = u64;

/// Event produced by [`AnimationController::tick`].
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationEvent<T: Animatable> {
    Value { id: AnimationId, value: T },
    Completed { id: AnimationId, value: T },
}

struct ActiveAnimation<T: Animatable> {
    id: AnimationId,
    animation: Animation<T>,
}

/// Owns and advances a collection of animations each frame.
///
/// The internal active list is backed by an [`alloc::vec::Vec`] that retains its
/// allocated capacity across frames; only `clear()` semantics are used.
pub struct AnimationController<T: Animatable> {
    active: Vec<ActiveAnimation<T>>,
    removals: Vec<usize>,
    next_id: AnimationId,
}

impl<T: Animatable> Default for AnimationController<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Animatable> AnimationController<T> {
    /// Create an empty controller.
    pub fn new() -> Self {
        Self {
            active: Vec::new(),
            removals: Vec::new(),
            next_id: 1,
        }
    }

    /// Start an animation and return its handle.
    pub fn start(&mut self, mut animation: Animation<T>) -> AnimationId {
        animation.start();
        let id = self.next_id;
        self.next_id += 1;
        self.active.push(ActiveAnimation { id, animation });
        id
    }

    /// Stop the animation with `id`. It will be removed on the next tick.
    pub fn stop(&mut self, id: AnimationId) -> bool {
        if let Some(entry) = self.active.iter_mut().find(|e| e.id == id) {
            entry.animation.stop();
            return true;
        }
        false
    }

    /// Pause the animation with `id`.
    pub fn pause(&mut self, id: AnimationId) -> bool {
        if let Some(entry) = self.active.iter_mut().find(|e| e.id == id) {
            entry.animation.pause();
            return true;
        }
        false
    }

    /// Resume the paused animation with `id`.
    pub fn resume(&mut self, id: AnimationId) -> bool {
        if let Some(entry) = self.active.iter_mut().find(|e| e.id == id) {
            entry.animation.resume();
            return true;
        }
        false
    }

    /// Restart the animation with `id` from the beginning.
    pub fn restart(&mut self, id: AnimationId) -> bool {
        if let Some(entry) = self.active.iter_mut().find(|e| e.id == id) {
            entry.animation.restart();
            return true;
        }
        false
    }

    /// Number of currently active animations.
    pub fn len(&self) -> usize {
        self.active.len()
    }

    /// Whether the controller has no active animations.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    /// Advance every running animation by `dt` and emit events to `on_event`.
    ///
    /// Completed and stopped animations are removed without allocating. The
    /// internal buffers keep their capacity across frames.
    pub fn tick<F: FnMut(AnimationEvent<T>)>(&mut self, dt: Duration, mut on_event: F) {
        self.removals.clear();

        for (index, entry) in self.active.iter_mut().enumerate() {
            match entry.animation.tick(dt) {
                Some(AnimationTick::Running(value)) => {
                    on_event(AnimationEvent::Value {
                        id: entry.id,
                        value,
                    });
                }
                Some(AnimationTick::Completed(value)) => {
                    on_event(AnimationEvent::Completed {
                        id: entry.id,
                        value,
                    });
                    self.removals.push(index);
                }
                None => {
                    if entry.animation.state() == AnimationState::Stopped {
                        self.removals.push(index);
                    }
                }
            }
        }

        // Remove completed/stopped entries in reverse order so indices stay valid.
        for index in self.removals.iter().rev().copied() {
            self.active.swap_remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::Cell;

    #[test]
    fn linear_animation_completes() {
        let completed = Rc::new(Cell::new(false));
        let cb = completed.clone();
        let mut anim = Animation::new(0.0_f32, 1.0_f32, Duration::from_millis(100))
            .on_complete(move || cb.set(true));
        anim.start();

        let result = anim.tick(Duration::from_millis(100));
        assert!(matches!(result, Some(AnimationTick::Completed(1.0))));
        assert!(completed.get());
        assert_eq!(anim.state(), AnimationState::Completed);
    }

    #[test]
    fn linear_animation_progresses_each_frame() {
        let mut anim = Animation::new(0.0_f32, 10.0_f32, Duration::from_millis(100));
        anim.start();

        let half = anim.tick(Duration::from_millis(50));
        assert!(matches!(half, Some(AnimationTick::Running(v)) if v == 5.0));
        assert_eq!(anim.state(), AnimationState::Running);
    }

    #[test]
    fn ease_in_out_bounds() {
        let curve = AnimationCurve::EaseInOut;
        let steps = 11;
        for i in 0..steps {
            let t = i as f32 / (steps - 1) as f32;
            let v = curve.ease(t);
            assert!(
                v >= 0.0 && v <= 1.0,
                "ease-in-out produced out-of-bounds value {v} at t={t}"
            );
        }
    }

    #[test]
    fn spring_overshoots() {
        let curve = AnimationCurve::Spring;
        // The spring curve should exceed 1.0 during the first overshoot.
        let v = curve.ease(0.2);
        assert!(v > 1.1, "expected spring overshoot above 1.1, got {v}");
    }

    #[test]
    fn cancellation_removes_animation() {
        let mut controller = AnimationController::<f32>::new();
        let id = controller.start(Animation::new(0.0, 1.0, Duration::from_millis(100)));
        assert_eq!(controller.len(), 1);

        assert!(controller.stop(id));
        let mut events = Vec::new();
        controller.tick(Duration::from_millis(50), |e| events.push(e));
        assert!(events.is_empty());
        assert!(controller.is_empty());
    }

    #[test]
    fn restart_resets_and_completes() {
        let mut controller = AnimationController::<f32>::new();
        let id = controller.start(Animation::new(0.0, 1.0, Duration::from_millis(100)));

        let mut events = Vec::new();
        controller.tick(Duration::from_millis(60), |e| events.push(e));
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], AnimationEvent::Value { value, .. } if value > 0.0));

        assert!(controller.restart(id));
        events.clear();
        controller.tick(Duration::from_millis(100), |e| events.push(e));
        assert!(matches!(
            events.last(),
            Some(AnimationEvent::Completed { value: 1.0, .. })
        ));
    }

    #[test]
    fn concurrent_independent_animations() {
        let mut controller = AnimationController::<f32>::new();
        let short = controller.start(Animation::new(0.0, 1.0, Duration::from_millis(50)));
        let long = controller.start(Animation::new(10.0, 20.0, Duration::from_millis(100)));

        let mut events = Vec::new();
        controller.tick(Duration::from_millis(60), |e| events.push(e));

        let short_completed = events
            .iter()
            .any(|e| matches!(e, AnimationEvent::Completed { id, value: 1.0 } if *id == short));
        let long_running = events
            .iter()
            .any(|e| matches!(e, AnimationEvent::Value { id, value } if *id == long && *value > 10.0 && *value < 20.0));

        assert!(short_completed, "short animation should have completed");
        assert!(long_running, "long animation should still be running");

        events.clear();
        controller.tick(Duration::from_millis(50), |e| events.push(e));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AnimationEvent::Completed { id, value: 20.0 } if *id == long))
        );
        assert!(controller.is_empty());
    }

    #[test]
    fn color_animatable_interpolates() {
        let a = Color::from_rgb(0, 0, 0);
        let b = Color::from_rgb(255, 255, 255);
        let mid = a.lerp(&b, 0.5);
        assert_eq!(mid, Color::from_rgb(128, 128, 128));
    }

    #[test]
    fn transform_animatable_interpolates() {
        let a = Transform::identity();
        let b = Transform::translate(10.0, 20.0);
        let mid = a.lerp(&b, 0.5);
        assert_eq!(mid, Transform::translate(5.0, 10.0));
    }

    #[test]
    fn pause_and_resume_preserves_state() {
        let mut anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(100));
        anim.start();
        anim.tick(Duration::from_millis(40));
        let value_before = anim.value();

        anim.pause();
        anim.tick(Duration::from_millis(100));
        assert_eq!(anim.value(), value_before);
        assert_eq!(anim.state(), AnimationState::Paused);

        anim.resume();
        let result = anim.tick(Duration::from_millis(60));
        assert!(matches!(result, Some(AnimationTick::Completed(1.0))));
    }
}

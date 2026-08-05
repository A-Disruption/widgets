//! Shared open/close transitions for the overlay-backed widgets.
//!
//! Menus, popovers, dialogs and toasts all appear and disappear the same way:
//! a surface fades in while sliding a short distance out of whatever it is
//! anchored to, and reverses on the way out. This module owns that behaviour so
//! each widget only has to say *when* it opens and *where* it slides from.
//!
//! # What can actually be animated
//!
//! This version of iced has no compositing opacity — [`Renderer::with_layer`]
//! creates a layer but takes no alpha, and only images and SVGs carry an
//! `opacity` field. So a surface cannot simply be drawn at 40% and be done
//! with.
//!
//! What [`fade`] does instead is scale the alpha of the colors a widget draws
//! *itself*: its background, border, shadow, and the text color it passes down
//! to its children. Children that inherit their color — plain `text`, icons —
//! fade with the surface. A child that sets its own explicit color does not.
//! Over the ~120ms a menu takes to open, that difference is not perceptible,
//! but it is real, and it is why the default motion is short.
//!
//! Sliding, by contrast, is exact: it is a translation of the layout position
//! and applies to every pixel of the surface.
//!
//! # Cost
//!
//! A [`Transition`] only asks for redraws while it is actually moving, and
//! [`Motion::NONE`] short-circuits to instant, so a widget that opts out never
//! schedules a frame it does not need.
//!
//! [`Renderer::with_layer`]: iced::advanced::Renderer::with_layer

use iced::animation::{Animation, Easing};
use iced::time::{Duration, Instant};
use iced::{Background, Color, Vector};

use crate::anchor::Side;

/// How a surface animates in and out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    /// How long a full open or close takes.
    pub duration: Duration,
    /// The curve the transition follows.
    pub easing: Easing,
    /// How far the surface slides, in logical pixels.
    ///
    /// The surface starts this far towards whatever it is anchored to and
    /// travels outwards as it opens.
    pub slide: f32,
}

impl Motion {
    /// No animation: surfaces appear and disappear instantly.
    ///
    /// A [`Transition`] using this never requests a redraw.
    pub const NONE: Self = Self {
        duration: Duration::ZERO,
        easing: Easing::Linear,
        slide: 0.0,
    };

    /// A short, restrained transition suited to menus and popovers.
    pub const QUICK: Self = Self {
        duration: Duration::from_millis(120),
        easing: Easing::EaseOutCubic,
        slide: 6.0,
    };

    /// A longer transition suited to dialogs and toasts, which cover more of
    /// the screen and benefit from being easier to follow.
    pub const SMOOTH: Self = Self {
        duration: Duration::from_millis(220),
        easing: Easing::EaseOutCubic,
        slide: 12.0,
    };

    /// Sets how long a full open or close takes.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Sets the curve the transition follows.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Sets how far the surface slides, in logical pixels.
    pub fn slide(mut self, slide: f32) -> Self {
        self.slide = slide;
        self
    }

    /// Returns `true` when this [`Motion`] has nothing to animate.
    pub fn is_instant(&self) -> bool {
        self.duration.is_zero()
    }
}

impl Default for Motion {
    fn default() -> Self {
        Self::QUICK
    }
}

/// Tracks whether a surface is open, closed, or somewhere in between.
///
/// The point of this type is that a closing surface is still *visible*: the
/// widget has to keep producing and drawing it until the transition finishes,
/// even though it is logically already closed.
#[derive(Debug)]
pub struct Transition {
    animation: Animation<bool>,
    motion: Motion,
}

impl Transition {
    /// Creates a closed [`Transition`].
    pub fn new() -> Self {
        Self {
            animation: Animation::new(false)
                .duration(Motion::QUICK.duration)
                .easing(Motion::QUICK.easing),
            motion: Motion::QUICK,
        }
    }

    /// Applies `motion`, which may have changed since the last view.
    ///
    /// The widget owns its [`Motion`] and the state tree owns the
    /// [`Transition`], so they have to be reconciled each pass. Rebuilding only
    /// on an actual change keeps that free in the common case.
    pub fn sync(&mut self, motion: Motion) {
        if self.motion == motion {
            return;
        }

        self.animation = self
            .animation
            .clone()
            .duration(motion.duration)
            .easing(motion.easing);

        self.motion = motion;
    }

    /// Returns the [`Motion`] currently applied.
    pub fn motion(&self) -> Motion {
        self.motion
    }

    /// Starts opening, or continues if already open.
    pub fn open(&mut self, now: Instant) {
        if !self.animation.value() {
            self.animation.go_mut(true, now);
        }
    }

    /// Starts closing.
    ///
    /// The surface stays [`visible`](Self::is_visible) until the transition
    /// finishes.
    pub fn close(&mut self, now: Instant) {
        if self.animation.value() {
            self.animation.go_mut(false, now);
        }
    }

    /// Jumps straight to closed, skipping any transition.
    ///
    /// Used when a surface is torn down rather than dismissed — a view rebuild
    /// that removes it, for instance — where animating out would be a lie.
    pub fn finish_closed(&mut self) {
        self.animation = Animation::new(false)
            .duration(self.motion.duration)
            .easing(self.motion.easing);
    }

    /// Returns `true` when the surface should still be produced and drawn.
    ///
    /// This stays `true` through the whole closing transition.
    pub fn is_visible(&self, now: Instant) -> bool {
        self.animation.value() || self.animation.is_animating(now)
    }

    /// Returns `true` while the surface is on its way out.
    ///
    /// A closing surface is still drawn, but should no longer respond to
    /// anything.
    pub fn is_closing(&self, now: Instant) -> bool {
        !self.animation.value() && self.animation.is_animating(now)
    }

    /// Returns `true` while the transition is in flight.
    ///
    /// A widget should request another redraw for as long as this holds, and
    /// stop as soon as it does not.
    pub fn is_animating(&self, now: Instant) -> bool {
        self.animation.is_animating(now)
    }

    /// Returns how far open the surface is, from `0.0` to `1.0`.
    pub fn progress(&self, now: Instant) -> f32 {
        self.animation.interpolate(0.0, 1.0, now)
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::new()
    }
}

/// Scales the alpha of `color` by `progress`.
pub fn fade(color: Color, progress: f32) -> Color {
    Color {
        a: color.a * progress,
        ..color
    }
}

/// Scales the alpha of a [`Background`] by `progress`.
///
/// Gradients are returned untouched: they have no single alpha to scale, and
/// silently flattening one to a color would be worse than not fading it.
pub fn fade_background(background: Background, progress: f32) -> Background {
    match background {
        Background::Color(color) => Background::Color(fade(color, progress)),
        other => other,
    }
}

/// Returns the offset to draw a surface at while it transitions.
///
/// The surface starts pulled towards the [`Side`] of the anchor it sits on, so
/// it reads as emerging from the thing that opened it, and reaches zero when
/// fully open.
pub fn slide(side: Side, distance: f32, progress: f32) -> Vector {
    let remaining = distance * (1.0 - progress);

    match side {
        Side::Top => Vector::new(0.0, remaining),
        Side::Bottom => Vector::new(0.0, -remaining),
        Side::Left => Vector::new(remaining, 0.0),
        Side::Right => Vector::new(-remaining, 0.0),
    }
}

/// Returns the offset to draw a surface at while it slides in from a viewport
/// edge.
///
/// This is the mirror of [`slide`], and the distinction matters. A popover
/// emerges *from its trigger*, so it starts pulled towards the trigger and
/// travels outwards. A surface anchored to the window emerges *from beyond the
/// edge*: it starts outside the viewport and travels inwards. Using [`slide`]
/// for one of these makes it appear to pop off the edge towards the middle and
/// then vanish, rather than rolling out of the edge and back into it.
///
/// Pass the surface's own extent as `distance` to have it travel fully off
/// screen, which is what a sheet wants.
pub fn slide_from_edge(side: Side, distance: f32, progress: f32) -> Vector {
    let remaining = distance * (1.0 - progress);

    match side {
        Side::Top => Vector::new(0.0, -remaining),
        Side::Bottom => Vector::new(0.0, remaining),
        Side::Left => Vector::new(-remaining, 0.0),
        Side::Right => Vector::new(remaining, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_transition_is_closed_and_still() {
        let transition = Transition::new();
        let now = Instant::now();

        assert!(!transition.is_visible(now));
        assert!(!transition.is_animating(now));
        assert_eq!(transition.progress(now), 0.0);
    }

    #[test]
    fn opening_makes_the_surface_visible_immediately() {
        let mut transition = Transition::new();
        let now = Instant::now();

        transition.open(now);

        assert!(transition.is_visible(now));
        assert!(transition.is_animating(now));
        // Visible, but not yet drawn at full strength.
        assert_eq!(transition.progress(now), 0.0);
    }

    /// The reason this type exists: a closing surface has to outlive the
    /// decision to close it, or it would vanish instead of animating out.
    #[test]
    fn a_closing_surface_stays_visible_until_the_transition_ends() {
        let mut transition = Transition::new();
        let start = Instant::now();

        transition.open(start);

        let open = start + Duration::from_millis(200);
        assert_eq!(transition.progress(open), 1.0);

        transition.close(open);

        assert!(transition.is_visible(open));
        assert!(transition.is_visible(open + Duration::from_millis(60)));
        assert!(!transition.is_visible(open + Duration::from_millis(200)));
    }

    #[test]
    fn is_closing_holds_only_while_fading_out() {
        let mut transition = Transition::new();
        let start = Instant::now();

        transition.open(start);
        assert!(!transition.is_closing(start));

        let open = start + Duration::from_millis(200);
        transition.close(open);

        assert!(transition.is_closing(open));
        assert!(transition.is_closing(open + Duration::from_millis(60)));
        // Once it has finished, it is closed rather than closing.
        assert!(!transition.is_closing(open + Duration::from_millis(200)));
    }

    #[test]
    fn finishing_closed_drops_the_surface_without_animating() {
        let mut transition = Transition::new();
        let now = Instant::now();

        transition.open(now);
        transition.finish_closed();

        assert!(!transition.is_visible(now));
        assert!(!transition.is_animating(now));
    }

    /// A widget that opted out of animation must never schedule a frame.
    #[test]
    fn instant_motion_never_animates() {
        let mut transition = Transition::new();
        transition.sync(Motion::NONE);

        let now = Instant::now();
        transition.open(now);

        assert!(transition.is_visible(now));
        assert!(!transition.is_animating(now));
        assert_eq!(transition.progress(now), 1.0);
    }

    #[test]
    fn syncing_the_same_motion_twice_is_a_no_op() {
        let mut transition = Transition::new();
        let now = Instant::now();

        transition.open(now);
        transition.sync(Motion::QUICK);

        // The open is still in flight; syncing did not reset it.
        assert!(transition.is_animating(now));
        assert_eq!(transition.motion(), Motion::QUICK);
    }

    #[test]
    fn fading_scales_alpha_without_touching_the_color() {
        let color = Color::from_rgba(0.2, 0.4, 0.6, 0.8);
        let faded = fade(color, 0.5);

        assert_eq!(faded.a, 0.4);
        assert_eq!((faded.r, faded.g, faded.b), (color.r, color.g, color.b));
    }

    #[test]
    fn a_gradient_background_is_left_alone_rather_than_flattened() {
        let gradient = Background::Gradient(iced::Gradient::Linear(
            iced::gradient::Linear::new(0.0)
                .add_stop(0.0, Color::BLACK)
                .add_stop(1.0, Color::WHITE),
        ));

        assert_eq!(fade_background(gradient, 0.5), gradient);
    }

    /// The two slides are mirrors, and getting them the wrong way round is
    /// what makes a sheet pop towards the middle instead of rolling out of the
    /// edge it belongs to.
    #[test]
    fn sliding_from_an_edge_starts_outside_the_viewport() {
        // A sheet on the right edge starts off-screen to the right...
        assert_eq!(slide_from_edge(Side::Right, 10.0, 0.0), Vector::new(10.0, 0.0));
        // ...and arrives flush with it.
        assert_eq!(slide_from_edge(Side::Right, 10.0, 1.0), Vector::new(0.0, 0.0));

        // A sheet from the top starts above the window and rolls down.
        assert_eq!(slide_from_edge(Side::Top, 10.0, 0.0), Vector::new(0.0, -10.0));
        assert_eq!(slide_from_edge(Side::Bottom, 10.0, 0.0), Vector::new(0.0, 10.0));
        assert_eq!(slide_from_edge(Side::Left, 10.0, 0.0), Vector::new(-10.0, 0.0));
    }

    #[test]
    fn the_two_slides_are_opposites() {
        for side in [Side::Top, Side::Bottom, Side::Left, Side::Right] {
            let out = slide(side, 10.0, 0.0);
            let inward = slide_from_edge(side, 10.0, 0.0);

            assert_eq!(inward, Vector::new(-out.x, -out.y), "{side:?}");
        }
    }

    #[test]
    fn a_surface_slides_out_of_the_side_it_is_anchored_on() {
        // Anchored below: starts high, settles down into place.
        assert_eq!(slide(Side::Bottom, 10.0, 0.0), Vector::new(0.0, -10.0));
        assert_eq!(slide(Side::Bottom, 10.0, 1.0), Vector::new(0.0, 0.0));

        // Anchored to the right: starts left, travels right.
        assert_eq!(slide(Side::Right, 10.0, 0.0), Vector::new(-10.0, 0.0));
        assert_eq!(slide(Side::Left, 10.0, 0.0), Vector::new(10.0, 0.0));
        assert_eq!(slide(Side::Top, 10.0, 0.0), Vector::new(0.0, 10.0));
    }
}

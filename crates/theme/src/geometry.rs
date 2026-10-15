//! Placement and shape maths for the drawn parts of a skin, free of any UI
//! types so it can be tested here. Positions are expressed as a percentage of
//! the dialogue box plus a px offset, so they stay right when the user
//! resizes the window.

use std::f32::consts::PI;

use crate::manifest::{
    Anchor, ContinueMotion, DecorMotionKind, DecorMotionSpec, Edge, Len, TailSpec,
};

/// Vertical paint offset and opacity multiplier. Reduced motion lands on
/// the original pose, regardless of phase (the same pose as a preview).
pub fn decor_motion(motion: &DecorMotionSpec, t_s: f64, reduced: bool) -> (f32, f32) {
    if reduced || motion.amplitude == 0.0 {
        return (0.0, 1.0);
    }
    let phase = (t_s / f64::from(motion.period_s.max(0.5)) + f64::from(motion.phase))
        .rem_euclid(1.0) as f32;
    let angle = phase * 2.0 * PI;
    match motion.kind {
        DecorMotionKind::Float => (angle.sin() * motion.amplitude, 1.0),
        DecorMotionKind::Breathe => (0.0, 1.0 - motion.amplitude * (1.0 - angle.cos()) * 0.5),
    }
}

/// 0, ½ or 1 along each axis.
pub fn anchor_frac(anchor: Anchor) -> [f32; 2] {
    use Anchor::*;
    let x = match anchor {
        TopLeft | Left | BottomLeft => 0.0,
        Top | Center | Bottom => 0.5,
        TopRight | Right | BottomRight => 1.0,
    };
    let y = match anchor {
        TopLeft | Top | TopRight => 0.0,
        Left | Center | Right => 0.5,
        BottomLeft | Bottom | BottomRight => 1.0,
    };
    [x, y]
}

/// `percent`% of the box plus `px`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Offset {
    pub percent: f32,
    pub px: f32,
}

/// Where an anchored element's top-left corner goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub left: Offset,
    pub top: Offset,
}

/// The element's own `anchor` point lands on the box's, then moves by
/// `offset`. `size` may be px or a percentage of the box.
pub fn place(anchor: Anchor, offset: [f32; 2], size: [Len; 2]) -> Placement {
    let f = anchor_frac(anchor);
    let axis = |fx: f32, off: f32, len: Len| match len {
        Len::Px(w) => Offset {
            percent: fx * 100.0,
            px: off - fx * w,
        },
        Len::Percent(w) => Offset {
            percent: fx * (100.0 - w),
            px: off,
        },
    };
    Placement {
        left: axis(f[0], offset[0], size[0]),
        top: axis(f[1], offset[1], size[1]),
    }
}

/// A tail's node rectangle (px, relative to its side's anchor point) and the
/// triangles drawn inside it, as percentages of that rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct TailGeometry {
    /// Node rectangle `[x, y, w, h]` in px, relative to the anchor point on
    /// the box's border edge (x/y in screen axes).
    pub rect: [f32; 4],
    /// The border-coloured triangle.
    pub outer: [[f32; 2]; 3],
    /// The fill-coloured triangle, inset by the border and reaching back over
    /// the box border so the tail opens into the box.
    pub inner: [[f32; 2]; 3],
}

fn offset_line(a: [f32; 2], b: [f32; 2], d: f32, inside: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let mut n = [-dy / len, dx / len];
    // Point the normal towards the triangle's inside.
    let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    if (inside[0] - mid[0]) * n[0] + (inside[1] - mid[1]) * n[1] < 0.0 {
        n = [-n[0], -n[1]];
    }
    (
        [a[0] + n[0] * d, a[1] + n[1] * d],
        [b[0] + n[0] * d, b[1] + n[1] * d],
    )
}

fn intersect(p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], p4: [f32; 2]) -> [f32; 2] {
    let d = (p1[0] - p2[0]) * (p3[1] - p4[1]) - (p1[1] - p2[1]) * (p3[0] - p4[0]);
    if d.abs() < 1e-6 {
        return p2;
    }
    let t = ((p1[0] - p3[0]) * (p3[1] - p4[1]) - (p1[1] - p3[1]) * (p3[0] - p4[0])) / d;
    [p1[0] + t * (p2[0] - p1[0]), p1[1] + t * (p2[1] - p1[1])]
}

/// Geometry of `tail` on a box whose border on that side is `border` px.
pub fn tail_geometry(tail: &TailSpec, border: f32) -> TailGeometry {
    let b = border.max(0.0);
    let w = tail.width;
    let len = tail.length;
    // Work as if the tail hangs off the bottom edge: x along the side from the
    // base's start, y outwards from the box's outer edge.
    let tip_x = w * 0.5 + tail.lean;
    let min_x = 0.0f32.min(tip_x);
    let max_x = w.max(tip_x);
    let top = -b - 1.0;
    let outer = [[0.0, -b], [w, -b], [tip_x, len]];
    let inner = if b <= 0.0 {
        outer
    } else {
        let centroid = [(w + tip_x) / 3.0, (len - 2.0 * b) / 3.0];
        let (l1, l2) = offset_line(outer[0], outer[2], b, centroid);
        let (r1, r2) = offset_line(outer[1], outer[2], b, centroid);
        let base = ([0.0, top], [w, top]);
        let left = intersect(l1, l2, base.0, base.1);
        let right = intersect(r1, r2, base.0, base.1);
        let tip = intersect(l1, l2, r1, r2);
        [left, right, tip]
    };
    // Node rect in the "bottom" frame.
    let (rx, ry, rw, rh) = (min_x, top, max_x - min_x, len - top);
    let pct = |p: [f32; 2]| {
        [
            (p[0] - rx) / rw.max(1e-3) * 100.0,
            (p[1] - ry) / rh.max(1e-3) * 100.0,
        ]
    };
    let outer_p = outer.map(pct);
    let inner_p = inner.map(pct);
    // Rotate the frame onto the requested side. Percentages flip with it.
    let map = |p: [f32; 2]| match tail.side {
        Edge::Bottom => p,
        Edge::Top => [p[0], 100.0 - p[1]],
        Edge::Right => [p[1], p[0]],
        Edge::Left => [100.0 - p[1], p[0]],
    };
    let rect = match tail.side {
        Edge::Bottom => [rx, ry, rw, rh],
        Edge::Top => [rx, -ry - rh, rw, rh],
        Edge::Right => [ry, rx, rh, rw],
        Edge::Left => [-ry - rh, rx, rh, rw],
    };
    TailGeometry {
        rect,
        outer: outer_p.map(map),
        inner: inner_p.map(map),
    }
}

/// The continue indicator's animation at `t_s` seconds: vertical offset,
/// opacity and scale.
pub fn continue_motion(
    motion: ContinueMotion,
    bob_px: f32,
    period_s: f32,
    t_s: f32,
) -> (f32, f32, f32) {
    let period = period_s.max(0.1);
    let phase = (t_s / period).fract();
    match motion {
        ContinueMotion::Bob => (-(phase * PI).sin().abs() * bob_px, 1.0, 1.0),
        ContinueMotion::Blink => (0.0, if phase < 0.5 { 1.0 } else { 0.15 }, 1.0),
        ContinueMotion::Pulse => (0.0, 1.0, 1.0 + 0.12 * (phase * 2.0 * PI).sin()),
        ContinueMotion::None => (0.0, 1.0, 1.0),
    }
}

/// Which frame of a horizontal sprite sheet shows at `t_s`.
pub fn sprite_frame(frames: u32, fps: f32, t_s: f32) -> u32 {
    if frames <= 1 {
        return 0;
    }
    ((t_s * fps.max(0.5)).floor().max(0.0) as u64 % u64::from(frames)) as u32
}

//! Spacing v0: per-glyph-class default sidebearings from ink-profile
//! flatness.
//!
//! This is placeholder-honest spacing — a learned spacing head replaces it
//! later. The heuristic, per side of each glyph:
//!
//! 1. Flatten the placed outline and sample the ink edge (leftmost /
//!    rightmost crossing x) at 48 evenly spaced heights across the middle
//!    90% of the glyph's ink band.
//! 2. Measure each sample's recession from the extreme edge.
//! 3. Classify:
//!    - **Straight** — >= 60% of samples lie within 3% of the glyph height
//!      of the extreme (a flat stem: H, E left side).
//!    - **Round** — not straight, and the extreme is reached in the middle
//!      third of the band (a bulge: O, C, e).
//!    - **Diagonal** — otherwise: the extreme sits at the top or bottom
//!      (A, V, W, L right side, T sides).
//! 4. Assign the class default, as a fraction of UPM, snapped to the grid:
//!    straight 5.5%, round 3.2%, diagonal 1.4% (56 / 32 / 14 units at
//!    UPM 1024).
//!
//! Glyphs shorter than 8% of UPM (period, hyphen) always class as Round on
//! both sides — edge shape is meaningless at that size.

use img2bez::kurbo::{BezPath, PathEl, Point};
use img2bez::Outline;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeClass {
    Straight,
    Round,
    Diagonal,
}

/// Per-class sidebearing defaults in font units.
pub struct SpacingDefaults {
    straight: f64,
    round: f64,
    diagonal: f64,
}

impl SpacingDefaults {
    pub fn for_upm(upm: f64, grid: i32) -> Self {
        let snap = |v: f64| {
            if grid > 1 {
                (v / grid as f64).round() * grid as f64
            } else {
                v.round()
            }
        };
        SpacingDefaults {
            straight: snap(upm * 0.055),
            round: snap(upm * 0.032),
            diagonal: snap(upm * 0.014),
        }
    }

    pub fn sidebearing(&self, class: EdgeClass) -> f64 {
        match class {
            EdgeClass::Straight => self.straight,
            EdgeClass::Round => self.round,
            EdgeClass::Diagonal => self.diagonal,
        }
    }
}

/// Classify the left and right ink edges of a placed outline.
pub fn classify_edges(outline: &Outline, upm: f64) -> (EdgeClass, EdgeClass) {
    let polylines = flatten_outline(outline);
    let (mut y_min, mut y_max) = (f64::MAX, f64::MIN);
    for poly in &polylines {
        for p in poly {
            y_min = y_min.min(p.y);
            y_max = y_max.max(p.y);
        }
    }
    let height = y_max - y_min;
    if !height.is_finite() || height < upm * 0.08 {
        // Tiny mark (period, hyphen): edge shape is meaningless.
        return (EdgeClass::Round, EdgeClass::Round);
    }

    // Sample edges across the middle 90% of the ink band.
    const SAMPLES: usize = 48;
    let lo = y_min + height * 0.05;
    let hi = y_max - height * 0.05;
    let mut lefts = Vec::with_capacity(SAMPLES);
    let mut rights = Vec::with_capacity(SAMPLES);
    for i in 0..SAMPLES {
        let y = lo + (hi - lo) * i as f64 / (SAMPLES - 1) as f64;
        if let Some((l, r)) = edge_crossings(&polylines, y) {
            lefts.push((l, y));
            rights.push((r, y));
        }
    }
    if lefts.len() < 8 {
        return (EdgeClass::Round, EdgeClass::Round);
    }

    let tol = height * 0.03;
    let left = classify_side(&lefts, tol, y_min, y_max, false);
    let right = classify_side(&rights, tol, y_min, y_max, true);
    (left, right)
}

/// Classify one side from (edge_x, y) samples.
fn classify_side(
    samples: &[(f64, f64)],
    tol: f64,
    y_min: f64,
    y_max: f64,
    is_right: bool,
) -> EdgeClass {
    // Extreme edge x and the y at which it is reached.
    let mut extreme = samples[0].0;
    let mut extreme_y = samples[0].1;
    for &(x, y) in samples {
        let better = if is_right { x > extreme } else { x < extreme };
        if better {
            extreme = x;
            extreme_y = y;
        }
    }

    // Fraction of samples hugging the extreme.
    let near = samples
        .iter()
        .filter(|&&(x, _)| (x - extreme).abs() <= tol)
        .count();
    if near as f64 / samples.len() as f64 >= 0.60 {
        return EdgeClass::Straight;
    }

    // Bulge test: extreme reached in the middle third of the band.
    let band = y_max - y_min;
    let t = (extreme_y - y_min) / band;
    if (1.0 / 3.0..=2.0 / 3.0).contains(&t) {
        EdgeClass::Round
    } else {
        EdgeClass::Diagonal
    }
}

/// Horizontal ink extent at height `y`: (leftmost, rightmost) crossing x.
fn edge_crossings(polylines: &[Vec<Point>], y: f64) -> Option<(f64, f64)> {
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut hit = false;
    for poly in polylines {
        for pair in poly.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if (a.y - y) * (b.y - y) > 0.0 || a.y == b.y {
                continue;
            }
            let t = (y - a.y) / (b.y - a.y);
            let x = a.x + t * (b.x - a.x);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            hit = true;
        }
    }
    hit.then_some((min_x, max_x))
}

/// Flatten every contour of the outline to a closed polyline.
fn flatten_outline(outline: &Outline) -> Vec<Vec<Point>> {
    let mut polylines = Vec::new();
    for path in outline.to_bezpaths() {
        polylines.push(flatten_path(&path));
    }
    polylines
}

fn flatten_path(path: &BezPath) -> Vec<Point> {
    let mut points: Vec<Point> = Vec::new();
    let mut start: Option<Point> = None;
    img2bez::kurbo::flatten(path.elements().iter().copied(), 0.5, |el| match el {
        PathEl::MoveTo(p) => {
            start = Some(p);
            points.push(p);
        }
        PathEl::LineTo(p) => points.push(p),
        PathEl::ClosePath => {
            if let Some(s) = start {
                points.push(s);
            }
        }
        // flatten() only emits MoveTo/LineTo/ClosePath.
        _ => {}
    });
    points
}

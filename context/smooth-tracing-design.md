# Smooth Tracing: Design Document

A new default tracing mode for img2bez that produces smooth-only outlines with strict constraints, replacing the current corner-detection-heavy pipeline.

---

## Why Change

The current img2bez pipeline spends ~500 lines of code on corner detection (`alphamax`, `CURVATURE_TRANSITION_THRESHOLD`, false-corner filtering, multi-pass split-point analysis). This is where most bugs live. The corner detection was tuned against Virtua Grotesk (a geometric typeface with genuine sharp corners) but fails on scanned specimens where everything is slightly soft.

The insight: **if we make everything smooth, we eliminate the entire corner detection problem.** We can always add corners back as a post-processing step for glyphs that need them (A, V, W, E, etc.).

## Constraints

Every traced glyph must satisfy these rules:

### 1. All on-curve points are smooth

```xml
<point x="162" y="160" type="curve" smooth="yes"/>
```

No `type="line"` segments. No corner points. The outline is a continuous sequence of cubic bezier curves where the tangent is continuous at every on-curve point.

### 2. Handles are horizontal or vertical

The two off-curve points adjacent to each on-curve point must form an axis-aligned line with that on-curve point:

```
Good:  offcurve(100, 300) — oncurve(100, 200) — offcurve(100, 100)  ← vertical
Good:  offcurve(50, 200)  — oncurve(100, 200) — offcurve(150, 200)  ← horizontal
Bad:   offcurve(80, 250)  — oncurve(100, 200) — offcurve(120, 150)  ← diagonal
```

This is standard type design practice: handles extend horizontally or vertically from on-curve points. It makes outlines predictable and editable.

### 3. On-curve points on even integers (grid=2)

All on-curve point coordinates are multiples of 2. Off-curve handles may be fractional (preserving curve accuracy) but should favor even integers where possible.

### 4. Powers of 2 preferred

Where structurally meaningful, coordinates should use values from the pattern: 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024. This applies especially to:
- Sidebearings (64 units default)
- Stem widths
- Overshoot amounts
- Vertical metrics (ascender=832, descender=-256, cap=768, x-height=576)

## Algorithm

### Current pipeline (complex)

```
pixels → contour extraction → polygon approximation → corner detection
→ section classification (LINE vs CURVE) → cubic fitting → H/V snapping
→ grid snapping → direction fixing
```

The corner detection + section classification is 1000+ lines and the source of most quality issues.

### New pipeline (simple)

```
pixels → contour extraction → polygon approximation → extrema detection
→ smooth cubic fitting with H/V handles → grid snapping → direction fixing
```

### Step 1: Contour extraction (unchanged)

Dual-grid pixel-edge tracing + DP polygon approximation. This produces a polygon that closely follows the glyph outline. Keep this as-is.

### Step 2: Extrema detection (replaces corner detection)

Instead of trying to classify vertices as corners or smooth points, find the **extrema** of the polygon outline: points where the outline reaches its topmost, bottommost, leftmost, or rightmost extent.

These are the natural locations for on-curve points in type design. At an extremum, the curve's tangent is exactly horizontal (for top/bottom extrema) or vertical (for left/right extrema), which means the handles from that point are automatically H/V.

```
For each contour:
  1. Walk the polygon vertices
  2. Find local extrema: points where the direction changes from
     "going up" to "going down" (y-max), or "going right" to "going left" (x-max), etc.
  3. These become on-curve point locations
```

Type design convention: a well-drawn "O" has exactly 4 on-curve points — one at each extreme (top, bottom, left, right). The smooth tracing should produce the same.

### Step 3: Smooth cubic fitting with H/V handles

Between each pair of consecutive extrema points, fit a single cubic bezier:

```
Given:
  P0 = on-curve point (extremum)
  P3 = next on-curve point (next extremum)
  P0 is a y-extremum → handle from P0 must be horizontal
  P3 is an x-extremum → handle into P3 must be vertical

Solve for:
  P1 = P0 + (h0, 0)    ← horizontal handle from P0
  P2 = P3 + (0, -h3)   ← vertical handle into P3
  where h0 and h3 are the handle lengths
```

The handle lengths (h0, h3) are the only free parameters. Optimize them to minimize the distance between the cubic bezier and the polygon vertices between P0 and P3.

This is a **2-parameter optimization** per segment (handle lengths only), compared to the current pipeline which has a complex multi-pass grid search. It's simpler and more constrained = easier to get right.

### Step 4: Grid snapping + powers of 2

Snap on-curve points to grid=2 (even integers). For handle lengths, prefer powers of 2 (8, 16, 32, 64, 128) when within tolerance.

### Step 5: Direction fixing (unchanged)

Ensure outer contours are CCW and counters are CW.

## What About Sharp Corners?

Some glyphs genuinely need corners: A (apex), V (bottom), W (inner angles), E/F/L/T (right angles), Z (diagonal junctions).

**Phase 1:** Ignore corners entirely. Produce smooth-only outlines. For scanned specimens, the "corners" are already softened by printing/photography, so smooth curves are an accurate representation.

**Phase 2 (later):** Add a post-processing step that identifies the sharpest smooth points (where the angle between incoming and outgoing tangents is smallest) and optionally converts them to corners. This is simpler than the current approach because we start from known-good smooth outlines and make targeted changes, rather than trying to detect corners from noisy polygon data.

## Implementation Checklist

### Phase 1: Smooth-only tracing (make it work)

- [ ] Add `smooth_only: bool` field to `TracingConfig` (default `true`)
- [ ] Implement `find_extrema()` in `vectorize/` — walk polygon, find local x/y min/max
- [ ] Implement `fit_smooth_cubic()` — given two extrema and H/V handle constraint, find optimal handle lengths
- [ ] Wire into the main pipeline: when `smooth_only=true`, skip corner detection and section classification, use extrema + smooth fitting instead
- [ ] Ensure all on-curve points get `smooth="yes"` in the norad output
- [ ] Add tests: trace a circle (should produce 4 on-curve points at extrema), trace a rectangle (should produce 4 on-curve points at corners, all smooth with short handles)

### Phase 2: Make it the default

- [ ] Run autoresearch on both clean (Virtua Grotesk) and scan reference sets
- [ ] Compare IoU: smooth-only vs current pipeline
- [ ] If smooth-only wins (expected for scans), make `smooth_only=true` the default
- [ ] Keep `smooth_only=false` available via `--no-smooth-only` flag for backward compat
- [ ] Update CLI defaults in img2bez main.rs

### Phase 3: Grid discipline

- [ ] After smooth fitting, snap on-curve points to grid=2
- [ ] Snap handle lengths to nearest power of 2 when within tolerance
- [ ] Add `prefer_powers_of_two: bool` to TracingConfig
- [ ] Measure impact on IoU (should be minimal — snapping changes are small)

### Phase 4: Optional corner recovery (later)

- [ ] Implement `detect_sharp_smooth_points()` — find smooth points where the tangent angle change exceeds a threshold
- [ ] Implement `convert_to_corner()` — split a smooth point into a corner (two separate handle directions)
- [ ] Add `corner_threshold` parameter — angle below which smooth points become corners
- [ ] Test on geometric glyphs (A, V, E, Z) where corners matter

### Phase 5: Integration with img2ufo

- [ ] Update img2ufo's TracingConfig construction to use `smooth_only: true`
- [ ] Verify the Runebender background image alignment still works
- [ ] Run fontspector on the output
- [ ] Update autoresearch reference sets

## Key Files to Modify

| File | Change |
|------|--------|
| `img2bez/src/config.rs` | Add `smooth_only: bool` to TracingConfig |
| `img2bez/src/vectorize/mod.rs` | Branch on `smooth_only`: extrema path vs current path |
| `img2bez/src/vectorize/extrema.rs` | New file: find extrema on polygon |
| `img2bez/src/vectorize/smooth_fit.rs` | New file: fit smooth cubics with H/V handles |
| `img2bez/src/vectorize/curve.rs` | Existing file: keep as `smooth_only=false` path |
| `img2bez/src/main.rs` | Add `--no-smooth-only` flag |
| `img2ufo/src/ufo_builder.rs` | Set `smooth_only: true` in TracingConfig |

## Expected Benefits

1. **Simpler code.** Extrema detection + 2-parameter fitting replaces 1000+ lines of corner detection and multi-pass classification.
2. **Better scan results.** No false corners from pixel noise. Every curve segment gets a good fit because the constraints prevent pathological handle positions.
3. **Type-design-correct outlines.** On-curve points at extrema with H/V handles is how professional type designers draw. The output is immediately editable.
4. **Faster autoresearch.** Fewer parameters to tune (no alphamax, no SHORT_SECTION_TOLERANCE, no CURVATURE_TRANSITION_THRESHOLD). The main parameters are just `fit_accuracy` and the extrema detection sensitivity.
5. **Consistent with the power-of-2 grid.** The H/V handle constraint + even-integer grid produces clean, machine-readable outlines that tools can reason about.

## Reference

- The smooth-only constraint matches how fonts like [Open Gate Naskh](https://github.com/eliheuer/open-gate-naskh) are drawn
- The H/V handle rule is standard in type design tooling (RoboFont, Glyphs, FontLab all encourage this)
- The extrema-at-on-curve rule is an OpenType recommendation and a fontspector check (`outline_direction` and `points_at_extrema`)
- The power-of-2 grid follows the [Virtua Grotesk](https://github.com/eliheuer/virtua-grotesk) system (1024 UPM, 64-unit structural grid)

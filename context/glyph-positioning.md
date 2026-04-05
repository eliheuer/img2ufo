# Glyph Positioning: Uniform Scaling and Baseline Alignment

How to correctly position glyphs within the UPM rectangle so they match the proportions and baseline of the original type specimen.

---

## The Problem

Currently, each glyph PNG is traced independently with the same `target_height` (1088 units). This means every glyph image is stretched to fill the full ascender-to-descender range, regardless of its actual size in the specimen:

```
Source specimen:                   Current output (WRONG):

  A a        ← different heights    A a    ← both same height!
  ___        ← shared baseline      ___    ← no baseline relationship
```

A 173px uppercase "A" and a 127px lowercase "a" both get scaled to 1088 units. The "a" appears as tall as the "A" in the font. Descenders, overshoots, and relative proportions are all lost.

## What We Actually Need

All glyphs from the same specimen must share:
1. **One scale factor** — so a 173px "A" and a 127px "a" maintain their size ratio
2. **One baseline** — so uppercase letters sit on y=0 and descenders go below

## Data We Already Have

The `manifest.json` from img2glyph contains everything we need:

```json
{
  "id": "glyph_0001",
  "file": "A.png",
  "bbox": { "x": 127, "y": 55, "w": 158, "h": 173 },
  "row": 0,
  "glyph_name": "A",
  "unicode": "U+0041"
}
```

- `bbox.y` + `bbox.h` = bottom of glyph in source image (in pixels, y-down)
- `row` = which text line the glyph belongs to
- Uppercase letters in each row define the baseline (their bottom = baseline)
- The tallest uppercase letter defines the scale

## Algorithm

### Step 1: Compute the uniform scale

Find the tallest uppercase letter. Its pixel height maps to cap-height (768 units):

```
tallest_cap_px = max(bbox.h for glyphs where glyph_name in A-Z)
uniform_scale = cap_height / tallest_cap_px
```

Example from test.png: tallest uppercase H = 175px → `scale = 768 / 175 ≈ 4.39 units/px`

### Step 2: Determine the baseline per row

For each row, the baseline is the average bottom-edge y-coordinate of the uppercase letters in that row:

```
baseline_y[row] = mean(bbox.y + bbox.h for uppercase glyphs in row)
```

For rows with no uppercase letters (pure lowercase or digit rows), the baseline can be inferred from the row above: same baseline_y adjusted by the row offset.

A simpler heuristic that works for most specimens: use the **median bottom edge** of all non-descender glyphs in each row (A-Z, a-z excluding g/j/p/q/y, 0-9).

### Step 3: Compute per-glyph target_height and y_offset

For each glyph, the crop image has pixel dimensions `(crop_w, crop_h)`. The crop covers source-image rows from `crop_top` to `crop_bottom`:

```
padding = 10  (from img2glyph config)
crop_top    = max(0, bbox.y - padding)
crop_bottom = min(image_h, bbox.y + bbox.h + padding)
crop_h      = crop_bottom - crop_top
```

To achieve uniform scaling in img2bez:
```
target_height = crop_h * uniform_scale
```

For baseline alignment, the baseline is at `baseline_y[row]` in source-image coords. In the crop image (y-down), the baseline is at pixel:
```
baseline_in_crop = baseline_y[row] - crop_top  (from top of crop)
```

img2bez uses a y-up coordinate system where y=0 is the bottom of the image. The baseline in img2bez coords:
```
baseline_from_bottom = crop_h - baseline_in_crop
```

After scaling, this should map to y=0 (the font baseline):
```
baseline_from_bottom * uniform_scale + y_offset = 0
y_offset = -(baseline_from_bottom * uniform_scale)
```

### Summary per glyph

```
target_height = crop_h * uniform_scale
y_offset      = -(crop_h - (baseline_y - crop_top)) * uniform_scale
```

Or equivalently:
```
y_offset = -(crop_bottom - baseline_y) * uniform_scale
```

## Worked Example

From test.png with cap-height = 768, padding = 10:

| Glyph | bbox.y | bbox.h | crop_h | baseline_y | target_height | y_offset |
|-------|--------|--------|--------|------------|---------------|----------|
| A     | 55     | 173    | 193    | 234        | 847           | -83      |
| H     | 56     | 175    | 195    | 234        | 856           | -53      |
| a     | 590    | 127    | 147    | 714        | 645           | -92      |
| g     | 595    | 188    | 208    | 714        | 913           | 17       |
| p     | 817    | 187    | 207    | ~1000      | 909           | ~-27     |

With `uniform_scale = 768 / 175 ≈ 4.39`:
- "A" (173px) → 760 units tall (close to cap-height ✓)
- "a" (127px) → 558 units tall (close to x-height ✓)
- "g" descender extends ~69px below baseline → ~303 units below (fits in descender ✓)

## Implementation

### Where to change code

**img2ufo `src/ufo_builder.rs`** — the main change:

Before the tracing loop, compute `uniform_scale` and per-row `baseline_y` from the manifest. Then for each glyph, compute individual `target_height` and `y_offset` instead of using the same values for all glyphs.

```rust
// Pseudocode:
let uniform_scale = compute_scale(&manifest, config.cap_height);
let baselines = compute_baselines(&manifest);

for entry in &manifest.glyphs {
    let (target_height, y_offset) = compute_glyph_metrics(
        entry, uniform_scale, &baselines, config.padding
    );
    let tracing_config = TracingConfig {
        target_height,
        y_offset,
        grid: config.grid,
        ..Default::default()
    };
    // trace with per-glyph metrics...
}
```

**img2bez** — no changes needed. Already supports per-glyph `target_height` and `y_offset`.

**img2glyph** — ideally add `source_width` and `source_height` to the manifest so we know the full image dimensions for clamping calculations. Otherwise we can read them from `input.png`.

### What about the background images in Runebender?

The `BackgroundImage::load()` function in Runebender also needs to use the same uniform scale and baseline offset. Currently it scales each image to fit ascender-to-descender, which has the same problem.

With the uniform scale approach, the background image for each glyph should be positioned using the same `y_offset` and `target_height` that img2bez used for tracing. This ensures the background image aligns exactly with the traced outlines.

This could be done by:
1. Storing the per-glyph `target_height` and `y_offset` in the manifest after tracing
2. Having Runebender read these values when loading background images
3. Or: computing them the same way from the manifest bbox data

## Edge Cases

**Mixed rows (uppercase + lowercase on same line):** Row 3 in test.png has "WXYZ abcdefg". The baseline comes from the uppercase letters (W, X, Y, Z). The lowercase letters use the same baseline — "a" sits on it, "g" descends below it.

**Rows with only lowercase/digits:** If a row has no uppercase letters (e.g., "hijklmnopqrstuv"), the baseline can be estimated from the glyph tops: lowercase letters without ascenders (a, c, e, m, n, o, r, s, u, v, w, x, z) have tops near the x-height. The baseline = glyph_top + (glyph_height * x_height / cap_height).

Or simpler: use the baseline from an adjacent row that does have uppercase letters.

**Variable baseline across rows:** In a real photograph, each row might have a slightly different baseline due to perspective distortion. Using per-row baselines (Step 2) handles this.

**Overshoots:** Round glyphs (O, C, S, e, o) typically extend slightly above cap-height and below baseline. This is expected — the scale should be set from the H (which has no overshoot), not from O.

**Reference glyph selection:** H is the best reference for cap-height (flat top and bottom, no overshoot). If H is not in the specimen, use E, F, or any flat-topped uppercase letter.

## Do We Need LLMs for This?

**No.** The algorithm is fully deterministic:
1. Find uppercase letters (by Unicode range or glyph_name)
2. Find the tallest one → scale
3. Find baseline from uppercase bottom edges → position
4. Apply per-glyph target_height and y_offset

No visual interpretation needed. The manifest has all the required data.

The only case where an LLM might help: if the specimen is not standard Latin and we can't automatically identify which glyphs are "uppercase" to derive the baseline. But for Latin specimens, Unicode ranges handle this.

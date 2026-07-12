# Vertical placement & metrics — how img2ufo puts traces in the box

2026-07-12. Validated end-to-end on a high-res specimen (7-glyph
"Antwerp" test). This documents what the assemble stage already does,
how to drive it, and its current limits.

## What the assembler does (src/ufo_builder.rs, MetricsSystem)

The segmenter's manifest.json is the metrics spec — no per-glyph
decisions, no ML:

1. **Uniform scale**: uppercase glyphs vote on a reference cap height
   in pixels (H preferred — flat top and bottom, no overshoot; else
   the median). `scale = --cap-height / reference_px`.
2. **Per-row baselines**: median bottom edge of the row's uppercase
   letters that sit flat (J and Q excluded). Rows without uppercase
   fall back to the median bottom of non-descender glyphs, then to the
   nearest row's baseline.
3. **Every traced glyph** gets the same uniform scale and its row's
   baseline offset. Descenders hang, overshoots survive (they are in
   the ink, not the metrics): in the Antwerp test the e landed at
   y −10..470 and the p at −230..474 with nobody telling the pipeline
   about overshoot — it was measured through the trace.
4. Coordinates snap to `--grid` (default 2: the power-of-two grid at
   `--upm 1024`).

## Recipe

    # one shot from a specimen image (segmentation included):
    img2ufo scan.png --labels labels.json \
        --family "My Face" --cap-height 688 --x-height 472 \
        --descender -232 --output MyFace-Regular.ufo

    # or reuse an existing img2glyph run (segmentation skipped when
    # manifest.json is present in --glyph-dir):
    img2ufo scan.png --glyph-dir glyphs/ --labels labels.json ...

labels.json maps segment ids to codepoints:

    {"glyph_0001": {"unicode": "U+0041"}, ...}

Uppercase labels matter: scale and baselines are voted by A–Z. A sheet
with no labeled uppercase still assembles, but from the weaker
non-descender fallback.

Choose metric targets on the grid (multiples of 8/16 at UPM 1024) and
proportional to the specimen: measure cap and x-height pixels from the
manifest, keep their ratio, snap to the grid. Antwerp test: cap 588px,
x 406px -> `--cap-height 688 --x-height 472` (ratio preserved, both
on the 8-grid).

## Current limits (state 2026-07-12)

- **x-height / descender / ascender are flags, not derived.** Baseline
  and scale come from the manifest; the other verticals are whatever
  you pass (defaults otherwise — the Antwerp test showed xHeight 576
  default vs ~474 measured). TODO: derive them by the same voting
  (lowercase-row tops for x-height; p/g/q/y bottoms for descender;
  b/d/h/k/l tops for ascender) and let flags override.
- **Spacing is provisional** (ink bounds + defaults; see spacing.rs) —
  real sidebearings are a separate pass.
- **The QA gate is honest**: a partial charset FAILs the googlefonts
  profile (glyph_coverage, case_mapping, render_own_name). For pilot
  runs on partial sheets, expect a FAIL exit after the TTF is built —
  the UFO/TTF are still written.
- **Segmenter threshold window must exceed stroke width**: img2glyph's
  adaptive threshold (default --block-radius 15) hollows out glyphs
  whose stems are wider than the window — large/high-res sources need
  `--block-radius 200`-class values (or: auto-set from median glyph
  size; TODO upstream in img2glyph). Symptom: traces come out as
  outline bands ("fill shapes").

## Worked example (reproduced 2026-07-12)

High-res specimen (3420x1000, ~588px caps), 7 glyphs A n t w e r p:

    img2glyph process antwerp.png -o glyphs --block-radius 200 --max-area 900000
    img2ufo antwerp.png --glyph-dir glyphs --labels labels.json \
        --family "Antwerp Test" --cap-height 688 \
        --output AntwerpTest-Regular.ufo

Result: A ink 0..686 (cap 688), n on the baseline exactly, e overshoot
-10, p descender -230; UFO + compiled TTF + completion worklist +
fontspector report. Traced point economy at this resolution class:
e = 12 on-curve points, n = 28 — near hand-drawn.

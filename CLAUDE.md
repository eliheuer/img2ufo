# img2ufo — agent guide

One image of type in, one correctly-placed UFO (+ compiled TTF + QA
report) out. This file is the entry point for a fresh session; the
detailed docs it points at live in `docs/`.

## The validated recipe (image -> placed font, 2026-07-12)

```bash
# 1. Segment. CRITICAL: --block-radius must exceed the widest stroke
#    in pixels, or glyphs come out as hollow outline bands ("fill
#    shapes"). High-res sources (300px+ glyphs) need ~200.
img2glyph process sheet.png -o glyphs --block-radius 200 --max-area 900000

# 2. Label. Read the crops (or use manifest row/col order for known
#    alphabet sheets) and write labels.json:
#    {"glyph_0001": {"unicode": "U+0041"}, ...}
#    Label the UPPERCASE letters — vertical placement is voted by A-Z.

# 3. One shot: trace + place + assemble + compile + QA gate.
img2ufo sheet.png --glyph-dir glyphs --labels labels.json \
    --family "My Face" --cap-height 688 --x-height 472 --descender -232 \
    --output MyFace-Regular.ufo
```

Placement is automatic (see `docs/placement.md`): uniform scale voted
by the uppercase (H preferred), per-row baselines from uppercase
bottoms, descenders hang, optical overshoots survive in the ink.
Verified: traces land ON the baseline, caps AT cap height, on the
2-unit power-of-two grid at UPM 1024.

Pick metric targets from the manifest, not from thin air: measure the
cap and x-height in pixels (bbox voting), keep the specimen's ratio,
snap the targets to multiples of 8/16.

## Pitfalls (each cost real debugging time — do not rediscover)

- **Hollow "fill shape" traces** = segmenter threshold window smaller
  than the stroke width. Fix: bigger `--block-radius`. Check a crop
  visually before tracing 78 glyphs.
- **x-height / descender / ascender are NOT derived** (yet) — pass
  them explicitly or fontinfo gets defaults that contradict the ink.
  Baseline + scale ARE derived. (TODO: same voting, lowercase rows.)
- **Partial sheets FAIL the QA gate by design** (glyph_coverage,
  case_mapping, render_own_name need a full charset). The UFO and TTF
  are still written before the FAIL exit — a pilot run "failing" is
  usually a success; read which checks failed.
- **macOS screenshot filenames** contain a narrow no-break space
  (U+202F) before "AM/PM" — a typed regular space will not match.
  Glob or copy to a sane name first.
- **Viewing in Runebender web** (`runebender-serve <dir> --port N`):
  serve a directory containing ONLY the UFO under review — the editor
  can pick up a different font from a multi-font workspace. Use a
  fresh port per project; stale servers from old sessions squat on
  ports (check `lsof -nP -iTCP:<port> -sTCP:LISTEN`).
- **Trace quality is dominated by input resolution.** ~500px glyphs
  trace at hand-drawn point economy (e = 12 on-curve points); ~100px
  scans wobble. Get resolution first (the "bridge" in
  docs/pipeline.md), tune the tracer second.

## The pieces (all local, all Rust)

- `/Users/eli/GH/repos/img2glyph` — segmentation (`img2glyph process`,
  `img2glyph label`). Output: glyph PNGs + manifest.json (bboxes,
  row/col — this IS the metrics data).
- `/Users/eli/GH/repos/img2bez` — the tracer. Also usable single-glyph
  (`img2bez -i g.png -o font.ufo -n a -u 0061`); `--grid 2` snapping
  default; `--threshold` (global Otsu default); `--mode smooth` for
  all-smooth output; `stats` subcommand for input-adaptive settings.
- This repo — the orchestrator (`img2ufo`, see `--help`): assemble
  (placement + fontinfo + spacing v0), composites
  (`src/composites.rs`, worklist per docs/glyph-completion-harness.md),
  fontc compile, fontspector gate (Rust successor to fontbakery —
  never recommend fontbakery).

## Project context

- **Everything machine-made is ORANGE** (public.markColor "1,0.5,0,1")
  per the mark-color convention: green = human-approved (only humans
  assign it), yellow = trusted-not-graded, orange = machine draft,
  red = garbage. Per glyph, never per font.
- **Mekorot**: tri-script family (Hebrew + Arabic + specimen-traced
  Latin) unified under the smooth x powers-of-two-grid style —
  `docs/mekorot-style.md`. The restyler lives at
  `/Users/eli/GH/repos/mekorot/scripts/mekorot_style.py` (UFO -> UFO);
  run it AFTER placement.
- **Sources for tracing are raster images only** — never copy or
  extract outlines from digital font files; rename releases; vintage
  (25y+) specimen material is the clean corpus (Letraset scans:
  `/Users/eli/Desktop/letraset/`).
- Design decisions (which specimen, metric targets, style calls) are
  Eli's; agent autonomy covers engineering.
- Worked example with real numbers: `docs/placement.md`. Full stage
  map: `docs/pipeline.md`. GF compliance details:
  `docs/gf-compliance-checklist.md`.

# Scan Quality Autoresearch Program

## Goal

Maximize `mean_iou` (raster IoU %, higher is better) on scanned type specimens.

The metric comes from `run_scan_experiment.sh` which traces glyph PNGs extracted from real scans and compares the output against hand-corrected reference outlines.

Secondary metric: `mean_score` (weighted vector quality, 0.0–1.0) should improve or hold steady.

**Do not stop.** Run experiments continuously until manually interrupted.

---

## How this differs from img2bez autoresearch

The img2bez autoresearch at `../img2bez/autoresearch/` optimizes tracing of **clean digital** glyphs (rendered from Virtua Grotesk). This loop optimizes the **full scan-to-UFO pipeline** on real scanned specimens with paper texture, anti-aliasing, and threshold noise.

You can change code in **all three repos**: img2glyph, img2bez, and img2ufo.

---

## Setup ritual (once per session)

1. Read these files to understand the system:
   - `autoresearch/program.md` (this file)
   - `context/scan-quality-strategy.md` (root cause analysis + improvement plan)
   - `src/pipeline.rs`, `src/ufo_builder.rs` (img2ufo pipeline)
   - `../img2bez/src/bitmap.rs` (thresholding + hole filling)
   - `../img2bez/src/vectorize/curve.rs` (curve fitting constants)
   - `../img2glyph/src/segment.rs` (segmentation + masking)

2. Create a branch:
   ```bash
   git checkout -b autoresearch/scans-$(date +%b%d | tr A-Z a-z)
   ```

3. Run baseline:
   ```bash
   ./autoresearch/run_scan_experiment.sh > run.log 2>&1
   grep "mean_iou:\|mean_score:\|glyphs_ok:\|glyphs_failed:" run.log
   ```

4. Initialize `references/specimen-001/results.tsv`:
   ```
   commit	mean_iou	mean_score	glyphs_ok	status	description
   <hash>	XX.XX	0.XXX	N	baseline	initial parameters
   ```

---

## Experiment loop (repeat forever)

### Step 1 — Identify weaknesses

Look at per-glyph logs in `references/specimen-001/comparison/`:
```bash
# Find the 5 worst glyphs by IoU
for f in references/specimen-001/comparison/uni*.log; do
    name=$(basename "$f" .log)
    iou=$(grep "Raster IoU" "$f" | grep -oE '[0-9]+\.[0-9]+' | head -1)
    [ -n "$iou" ] && echo "$iou $name"
done | sort -n | head -5
```

Look at comparison images to understand what's wrong:
- `*_comparison.png`: source / traced / overlay (3-panel)
- `*_raster_diff.png`: green=both, red=traced-only, blue=reference-only

### Step 2 — Propose a hypothesis

Choose ONE change. Examples:

**A. Parameter experiment** (fast, no code change):
```bash
./autoresearch/run_scan_experiment.sh --smooth 2 --accuracy 6.0
```

**B. Threshold experiment** (img2bez bitmap.rs):
- Change Otsu to Sauvola
- Add Gaussian blur before threshold
- Adjust fill_small_holes cutoff

**C. Masking experiment** (img2glyph segment.rs):
- Adjust dilation kernel size
- Make kernel proportional to glyph size
- Add morphological closing

**D. Curve fitting experiment** (img2bez vectorize/curve.rs):
- Adjust alphamax, SHORT_SECTION_TOLERANCE, MIN_SECTION_CHORD
- Change smooth_iterations default
- Modify corner detection thresholds

### Step 3 — Run experiment

```bash
./autoresearch/run_scan_experiment.sh [flags] > run.log 2>&1
grep "mean_iou:\|mean_score:\|glyphs_ok:\|glyphs_failed:" run.log
```

### Step 4 — Keep or discard

- **Keep** (mean_iou improved): commit the change, log to results.tsv
- **Discard** (equal or worse): `git checkout -- ../img2bez/src/ ../img2glyph/src/ src/`, log to results.tsv
- **Crash** (build error, no metrics): revert, log as crash

### Step 5 — Log result

Append to `references/specimen-001/results.tsv`:
```
<commit>	<mean_iou>	<mean_score>	<glyphs_ok>	<keep|discard|crash>	<description>
```

---

## What you CAN change

- `../img2bez/src/` — tracing algorithm (bitmap, vectorize, cleanup, config)
- `../img2glyph/src/` — segmentation, masking, extraction
- `src/` — img2ufo pipeline, TracingConfig construction
- CLI flags passed to run_scan_experiment.sh

## What you CANNOT change

- `autoresearch/run_scan_experiment.sh`
- `autoresearch/program.md`
- `references/*/expected.ufo/` (ground truth)
- `references/*/input/` (input data)

## What you SHOULD NOT change

- `Cargo.toml` dependencies in any repo (no new crates)
- Public API signatures that would break the CLI

---

## Where to look for improvements

### Thresholding (img2bez src/bitmap.rs)
- Otsu fails on scans with paper texture — Sauvola or blur+Otsu may help
- `fill_small_holes` cutoff: too low = spurious counters, too high = real counters filled
- Gaussian pre-blur to remove high-frequency paper noise

### Masking (img2glyph src/segment.rs)
- Fixed dilation k=8 doesn't adapt to glyph size
- Proportional: `k = clamp(glyph_height / 20, 4, 12)`
- Morphological closing before connected components to bridge broken strokes

### Curve fitting (img2bez src/vectorize/curve.rs)
- `alphamax`: current 0.80 was tuned for geometric type, scans may want 0.9+
- `smooth_iterations`: 1 for clean input, scans may want 2-3
- `fit_accuracy`: 4.0 for clean, scans may want 6.0-10.0 (looser fit)
- `SHORT_SECTION_TOLERANCE`: affects small curved sections
- `CURVATURE_TRANSITION_THRESHOLD`: straight vs curved detection

### Post-processing (img2bez src/cleanup/)
- Small contour removal (area < 1% of largest)
- Light curve smoothing (Laplacian, factor 0.1)
- Contour simplification

### Pipeline (img2ufo src/ufo_builder.rs)
- TracingConfig construction: scan-optimized defaults vs clean-input defaults
- Per-glyph parameter overrides

---

## Simplicity criterion

- 0.5% IoU gain with a 3-line constant change: **worth it**
- 0.1% IoU gain with 50 new lines: **not worth it**
- Deleting code that achieves equal results: **always a win**

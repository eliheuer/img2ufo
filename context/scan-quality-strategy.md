# Scan-to-UFO Quality Strategy

Working document for improving the img2glyph → img2bez → img2ufo pipeline output on scanned type specimens.

---

## Why Output Looks Wrong

Five failure modes, each traceable to a specific stage:

**A. Dirty binarization.** img2glyph uses adaptive threshold (block_radius=15), img2bez re-thresholds with Otsu. On scans with paper texture, Otsu clips strokes or opens false holes. The `fill_small_holes` function helps but uses a fixed cutoff.

**B. Component masking artifacts.** Fixed dilation kernel (k=8). Too small clips edges, too large bleeds neighbors. Disconnected parts (dot of i) get separate labels.

**C. Noisy contours → bad polygons.** DP polygon assumes clean pixel edges. Scan noise → extra vertices → false corners → wrong curves.

**D. Parameters tuned for clean digital input.** Optimized against Virtua Grotesk (96.72% IoU). Defaults overfit to that typeface.

**E. No measurement for scanned input.** No reference pairs for scans. Without measurement, improvement is guesswork.

---

## Reference Data Setup

### Directory structure

```
img2ufo/references/
  README.md                      # This section, expanded
  specimen-001/                  # One directory per specimen
    source.png                   # Original full-page scan
    metadata.json                # Font metrics + provenance
    input/                       # Cropped glyph PNGs (from img2glyph)
      A.png
      B.png
      ...
    expected.ufo/                # Hand-corrected outlines (ground truth)
      metainfo.plist
      fontinfo.plist
      glyphs/
        A_.glif
        B_.glif
        ...
    pipeline-output.ufo/         # Latest pipeline output (gitignored)
    comparison/                  # Visual diffs (gitignored)
      A_comparison.png
      A_raster_diff.png
      ...
    results.tsv                  # Autoresearch results for this specimen
```

### metadata.json

```json
{
  "name": "specimen-001",
  "description": "Bold serif type specimen, photographed from print",
  "source_file": "source.png",
  "font_metrics": {
    "units_per_em": 1000,
    "ascender": 800,
    "descender": -200,
    "x_height": 500,
    "cap_height": 700
  },
  "segmentation": {
    "min_area": 2000,
    "max_area": 50000,
    "block_radius": 15,
    "padding": 10
  },
  "glyphs_labeled": 75,
  "glyphs_corrected": 26,
  "date_created": "2026-04-03",
  "notes": "Uppercase A-Z hand-corrected in RoboFont"
}
```

### How to create a reference set

**Step 1 — Segment and label (we've already done this for test.png):**
```bash
img2glyph process source.png --output input/ --min-area 2000
img2glyph label input/manifest.json --assignments assignments.json
```

**Step 2 — Run pipeline to get initial UFO:**
```bash
img2ufo -i source.png -o pipeline-output.ufo --glyph-dir input/
```

**Step 3 — Hand-correct in a font editor:**
1. Open `pipeline-output.ufo` in RoboFont/Glyphs/FontForge
2. For each glyph, correct the outlines to match the source image
3. Save as `expected.ufo`

**Step 4 — Verify the reference set works:**
```bash
# From img2bez repo, trace one glyph and compare against reference:
img2bez -i references/specimen-001/input/A.png \
        -o /tmp/test.ufo -n A \
        --reference references/specimen-001/expected.ufo/glyphs/A_.glif
# Check the IoU and comparison images
```

### What makes a good reference

- **Correct contour count.** O has 2 contours (outer + counter). B has 3.
- **Clean curves.** No pixel staircase, smooth transitions at curve/line junctions.
- **Proper proportions.** Outlines match the source image at the stroke level.
- **Correct direction.** Outer CCW, counters CW (in Y-up font coordinates).
- **Reasonable point count.** Not too many (noise), not too few (lost detail).

You do NOT need pixel-perfect correction. The goal is "what a competent type designer would produce" — clean, usable outlines that match the source character's visual identity.

---

## Autoresearch Loop for Scans

### How it connects to Karpathy's autoresearch

Same pattern: `program.md` tells the LLM what to try → LLM edits code → `run_experiment.sh` measures → keep/discard → repeat.

The existing img2bez autoresearch already implements this with Virtua Grotesk. To extend for scans:

### run_scan_experiment.sh

A new script in `img2ufo/autoresearch/` that:

1. **Reads pre-existing input PNGs** (not rendered from a UFO — they come from img2glyph)
2. **Traces each glyph** with img2bez using current parameters
3. **Compares against reference .glif** using img2bez's `--reference` flag
4. **Reports mean_iou and mean_score** in the same parseable format

```bash
#!/bin/bash
# img2ufo/autoresearch/run_scan_experiment.sh
#
# Usage: ./run_scan_experiment.sh [extra img2bez flags...]
# Env:   REFERENCE_SET=references/specimen-001 (default)

set -euo pipefail

REFERENCE_SET="${REFERENCE_SET:-references/specimen-001}"
INPUT_DIR="$REFERENCE_SET/input"
EXPECTED_UFO="$REFERENCE_SET/expected.ufo"
METADATA="$REFERENCE_SET/metadata.json"
WORK_DIR="$REFERENCE_SET/comparison"
mkdir -p "$WORK_DIR"

# Build img2bez
(cd ../img2bez && cargo build --release)
IMG2BEZ="../img2bez/target/release/img2bez"

# Read font metrics from metadata.json
TARGET_HEIGHT=$(python3 -c "
import json; m=json.load(open('$METADATA'))['font_metrics']
print(m['ascender'] - m['descender'])")
Y_OFFSET=$(python3 -c "
import json; m=json.load(open('$METADATA'))['font_metrics']
print(m['descender'])")

# Iterate over labeled PNGs that have matching .glif references
total_iou=0; count=0; failed=0

for png in "$INPUT_DIR"/*.png; do
    name=$(basename "$png" .png)
    # Skip unlabeled glyphs (glyph_NNNN pattern)
    [[ "$name" =~ ^glyph_ ]] && continue

    # Find matching .glif in expected.ufo
    glif=$(python3 -c "
import plistlib, sys
p = plistlib.load(open('$EXPECTED_UFO/glyphs/contents.plist', 'rb'))
print(p.get('$name', ''))" 2>/dev/null)
    [ -z "$glif" ] && continue
    [ ! -f "$EXPECTED_UFO/glyphs/$glif" ] && continue

    echo "=== $name ==="
    $IMG2BEZ -i "$png" -o "$WORK_DIR/output.ufo" -n "$name" \
        --target-height "$TARGET_HEIGHT" --y-offset "$Y_OFFSET" \
        --reference "$EXPECTED_UFO/glyphs/$glif" \
        "$@" > "$WORK_DIR/${name}.log" 2>&1 || { failed=$((failed+1)); continue; }

    iou=$(grep "Raster IoU" "$WORK_DIR/${name}.log" | grep -oE '[0-9]+\.[0-9]+' | head -1)
    if [ -n "$iou" ]; then
        total_iou=$(echo "$total_iou + $iou" | bc)
        count=$((count+1))
        echo "  IoU: ${iou}%"
    fi
done

if [ $count -gt 0 ]; then
    mean_iou=$(echo "scale=2; $total_iou / $count" | bc)
    echo ""
    echo "mean_iou: ${mean_iou}%"
    echo "glyphs_ok: $count"
    echo "glyphs_failed: $failed"
fi
```

### program-scans.md

The LLM reads this to know how to run the scan autoresearch loop:

```markdown
# Scan Quality Autoresearch Program

## Goal
Maximize mean_iou on scanned type specimen reference sets.
The metric comes from `run_scan_experiment.sh` in `img2ufo/autoresearch/`.

## What you can change
- Any file under `img2bez/src/` (tracing algorithm)
- Any file under `img2glyph/src/` (segmentation/masking)
- Any file under `img2ufo/src/` (pipeline orchestration)
- CLI flags passed to run_scan_experiment.sh

## What you cannot change
- Reference sets (input PNGs + expected UFOs)
- run_scan_experiment.sh itself
- This file

## Experiment loop
1. Read per-glyph logs in references/specimen-001/comparison/
2. Identify the 3 worst-performing glyphs by IoU
3. Look at the _comparison.png and _raster_diff.png images
4. Hypothesize what's wrong (bad threshold? broken contour? wrong corners?)
5. Make ONE targeted change
6. Run: ./autoresearch/run_scan_experiment.sh > run.log 2>&1
7. Parse: grep "mean_iou:" run.log
8. Keep if improved, revert if not
9. Log to results.tsv
10. Repeat

## Key levers
- img2bez bitmap.rs: threshold method, fill_small_holes cutoff
- img2bez config.rs: smooth_iterations, alphamax, fit_accuracy, min_contour_area
- img2bez vectorize/curve.rs: algorithm constants
- img2glyph segment.rs: dilation kernel, block_radius
- img2ufo ufo_builder.rs: TracingConfig construction
```

### How LLMs use the reference data

**Visual comparison (Claude Code with vision):**
```
Read the file references/specimen-001/comparison/A_comparison.png
and references/specimen-001/input/A.png

Describe what's wrong with the traced output compared to the source.
What specific changes to the tracing algorithm would improve this glyph?
```

**Metric-driven analysis (any LLM):**
```
Read references/specimen-001/comparison/A.log

The raster IoU is 72%. The vector score breakdown shows:
- Contour count: 0.0 (expected 2, got 4)
- Shape distance: 0.45

This means the glyph has too many contours.
The extra contours are likely noise from thresholding.
Increase min_contour_area or improve hole filling.
```

**Batch analysis for parameter sweep:**
```
Run the experiment with these parameters and report results:
  --smooth 1 --alphamax 0.80 --accuracy 4.0
  --smooth 2 --alphamax 0.90 --accuracy 6.0
  --smooth 2 --alphamax 0.80 --accuracy 8.0
```

---

## Quick Start: Creating the First Reference Set

Using the test.png specimen we've been working with:

```bash
cd ~/GH/repos/img2ufo
mkdir -p references/specimen-001/input
mkdir -p references/specimen-001/comparison

# Copy current segmented glyphs as input
cp glyphs/*.png references/specimen-001/input/
cp test.png references/specimen-001/source.png

# Copy current pipeline output as starting point for correction
cp -r test_output.ufo references/specimen-001/pipeline-output.ufo

# Create metadata.json
cat > references/specimen-001/metadata.json << 'EOF'
{
  "name": "specimen-001",
  "description": "Bold serif type specimen from test.png",
  "source_file": "source.png",
  "font_metrics": {
    "units_per_em": 1000,
    "ascender": 800,
    "descender": -200,
    "x_height": 500,
    "cap_height": 700
  },
  "segmentation": {
    "min_area": 2000,
    "max_area": 50000
  },
  "glyphs_labeled": 75,
  "glyphs_corrected": 0,
  "date_created": "2026-04-03"
}
EOF

# Now open pipeline-output.ufo in a font editor,
# correct the outlines, and save as expected.ufo:
# cp -r references/specimen-001/pipeline-output.ufo references/specimen-001/expected.ufo
# open references/specimen-001/expected.ufo  # in your font editor
```

After you correct outlines for even a handful of glyphs and save as `expected.ufo`, the autoresearch loop can start measuring and optimizing overnight.

---

## Phased Improvement Plan

### Phase 0: Measurement Infrastructure (1-2 days)
Set up reference set structure, run_scan_experiment.sh, and baseline measurement.

### Phase 1: Pre-processing Improvements (3-5 days)
Better binarization (Sauvola threshold), adaptive dilation, adaptive hole filling.

### Phase 2: Parameter Tuning for Scans (3-5 days)
Automated parameter sweep using the autoresearch loop against scan references.

### Phase 3: Post-Trace Cleanup (2-4 days)
Small contour removal, curve smoothing, contour simplification.

### Phase 4: LLM-Assisted Quality (ongoing)
Claude Code drives the autoresearch loop. Visual comparison review. .glif correction.

---

## Critical Files

| File | Repo | Purpose |
|------|------|---------|
| `src/bitmap.rs` | img2bez | Binarization, hole filling |
| `src/config.rs` | img2bez | TracingConfig parameters |
| `src/vectorize/curve.rs` | img2bez | Algorithm constants (most tunable) |
| `src/segment.rs` | img2glyph | Dilation, component masking |
| `src/ufo_builder.rs` | img2ufo | TracingConfig construction |
| `autoresearch/run_experiment.sh` | img2bez | Clean-input experiment runner |
| `autoresearch/program.md` | img2bez | Clean-input autoresearch protocol |

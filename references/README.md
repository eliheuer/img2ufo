# Reference Sets

Each subdirectory is one reference set: a scanned type specimen with hand-corrected outlines that serve as ground truth for measuring and improving the pipeline.

## Structure

```
specimen-001/
  source.png              # Original full-page scan
  metadata.json           # Font metrics, segmentation params, notes
  assignments.json        # Unicode assignments (for re-labeling)
  input/                  # Cropped glyph PNGs from img2glyph
    A.png, B.png, ...
    manifest.json
  expected.ufo/           # ← YOU CREATE THIS: hand-corrected outlines
  pipeline-output.ufo/    # Latest pipeline output (starting point for correction)
  comparison/             # Generated: visual diffs from autoresearch (gitignored)
  results.tsv             # Autoresearch experiment history
```

## Creating a reference set

### 1. Segment and label

```bash
img2glyph process source.png --output input/ --min-area 2000
img2glyph label input/manifest.json --assignments assignments.json
```

### 2. Run pipeline

```bash
img2ufo -i source.png -o pipeline-output.ufo --glyph-dir input/
```

### 3. Hand-correct outlines

Open `pipeline-output.ufo` in a font editor (RoboFont, Glyphs, FontForge). For each glyph:
- Fix proportions to match the source image
- Clean up noise contours (delete small spurious shapes)
- Correct counter shapes (holes in O, e, a, etc.)
- Smooth curves, fix corners
- Ensure correct contour direction (outer CCW, counters CW)

Save as `expected.ufo`. Even 10 corrected glyphs is enough to start the autoresearch loop.

### 4. Run the autoresearch loop

```bash
cd ~/GH/repos/img2ufo
./autoresearch/run_scan_experiment.sh
```

## What makes a good reference

You do NOT need pixel-perfect correction. The goal is "what a competent type designer would produce" — clean, usable outlines that capture the character's visual identity from the source scan.

- Correct contour count (O=2, B=3, A=1, etc.)
- Clean curves with smooth transitions
- Proportions matching the source image
- Reasonable point count (not too many, not too few)

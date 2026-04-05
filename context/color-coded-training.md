# Color-Coded Training Data System

Use Runebender's mark colors as a quality axis for incremental font development and autoresearch.

---

## Color Spectrum

| Color | UFO markColor | Meaning |
|-------|---------------|---------|
| Red | `1,0.3,0.3,1` | Bad — pipeline output, needs full redraw |
| Orange | `1,0.6,0.2,1` | Bad but improving — has the right structure |
| Yellow | `1,0.9,0.2,1` | Almost good — minor corrections needed |
| Green | `0.3,0.7,0.3,1` | Good — hand-corrected, ready as reference |

## How It Works

### 1. Generate → everything starts red

When img2ufo generates a font, all traced glyphs get `markColor = "1,0,0,1"` (red). This is the raw pipeline output.

### 2. Hand-correct → promote to green

Open in Runebender. Correct outlines for a few key glyphs (start with H, O, n, o — they establish stems, rounds, and proportions). Mark corrected glyphs green.

### 3. Regenerate → greens survive

When the pipeline runs again, it **skips green glyphs** — they're already correct. Only red/orange/yellow glyphs get regenerated. This means you never lose your hand-drawn work.

### 4. Autoresearch → study the spectrum

The autoresearch loop can:
- Compare red glyphs (bad) with green glyphs (good) to learn what "good" looks like
- Measure improvement by counting color distribution changes
- Use green glyphs as reference pairs (input PNG → green .glif = ground truth)
- Track progress: "moved 5 glyphs from red to yellow this iteration"

### 5. Manual review → intermediate colors

After autoresearch improves the pipeline, open in Runebender and review:
- Glyphs that improved but aren't perfect → orange or yellow
- Glyphs that are now good → promote to green
- The color distribution shows overall progress at a glance

## Reference Set Structure

```
references/
  specimen-001/          # Bold serif (test.png)
  specimen-002/          # Light sans
  specimen-003/          # Script/handwriting
  ...
  specimen-009/          # Display/decorative

Each contains:
  source.png             # Original scan
  metadata.json          # Font metrics
  assignments.json       # Unicode labels
  input/                 # Cropped glyph PNGs (gitignored)
  expected.ufo/          # The working font (with color-coded glyphs)
    glyphs/
      H_.glif            # markColor: 0,1,0,1 (green = hand-drawn)
      O_.glif            # markColor: 0,1,0,1 (green = hand-drawn)
      A_.glif            # markColor: 1,0,0,1 (red = pipeline output)
      ...
```

## Pipeline Changes

### img2ufo: skip green glyphs on regeneration

In `ufo_builder.rs`, before tracing a glyph:

1. Check if the output UFO already exists
2. If so, load the existing glyph
3. If markColor is green (`0,1,0,1`), skip tracing — keep the existing glyph
4. Otherwise, trace and mark red

```
if existing_glyph has markColor green:
    keep existing glyph (don't overwrite)
    log: "keeping hand-drawn {name}"
else:
    trace from PNG
    set markColor to red
```

### img2ufo: set red on new traces

After tracing a glyph, set its markColor to red in the glyph lib:

```rust
glyph.lib.insert(
    "public.markColor".into(),
    Value::String("1,0,0,1".into()),
);
```

### autoresearch: use colors as signal

The `run_scan_experiment.sh` script can filter by color:

```bash
# Only evaluate green glyphs (reference quality)
GLYPH_FILTER="green" ./autoresearch/run_scan_experiment.sh

# Compare pipeline output against green references
# Green .glif = ground truth, red .glif = what pipeline produced
```

The LLM driving autoresearch can read the color distribution:
```bash
# Count colors in the font
python3 -c "
import plistlib, os
ufo = 'references/specimen-001/expected.ufo'
colors = {}
for f in os.listdir(f'{ufo}/glyphs'):
    if not f.endswith('.glif'): continue
    # parse glif, check markColor
    ...
print(colors)  # {'green': 5, 'red': 70, 'orange': 0, 'yellow': 0}
"
```

## Bootstrap Workflow

### Starting a new specimen (e.g., specimen-002):

```bash
# 1. Segment and label
img2glyph process specimen.png --output input/ --min-area 2000
# (use Claude to label)
img2glyph label input/manifest.json --assignments assignments.json

# 2. Generate initial UFO (everything red)
img2ufo -i specimen.png -o expected.ufo --glyph-dir input/

# 3. Open in Runebender and hand-draw 3-5 key glyphs
runebender expected.ufo --glyph-images input/
# Draw: H, O, n, o, zero — mark green when done

# 4. Run autoresearch overnight
# The loop studies the green vs red difference and tries to improve
./autoresearch/run_scan_experiment.sh > run.log 2>&1

# 5. Regenerate — greens survive, reds get new pipeline output
img2ufo -i specimen.png -o expected.ufo --glyph-dir input/
# Open in Runebender, review, promote improving glyphs to orange/yellow/green
```

### Minimum viable bootstrap: 5 glyphs

For each specimen, hand-draw these 5 to establish the design parameters:
- **H** — vertical stems, cap height, baseline
- **O** — round shapes, overshoot, counter
- **n** — x-height, lowercase stems, arch
- **o** — lowercase round, x-height overshoot
- **zero** — numeral proportions

These 5 give the autoresearch loop enough signal to optimize the rest.

## 9 Training Datasets

| # | Name | Type | Source |
|---|------|------|--------|
| 1 | specimen-001 | Bold serif | test.png (current) |
| 2 | specimen-002 | Light sans | TBD |
| 3 | specimen-003 | Script/calligraphy | TBD |
| 4 | specimen-004 | Geometric sans | TBD |
| 5 | specimen-005 | Old-style serif | TBD |
| 6 | specimen-006 | Slab serif | TBD |
| 7 | specimen-007 | Display/decorative | TBD |
| 8 | specimen-008 | Monospace | TBD |
| 9 | specimen-009 | Handwriting | TBD |

Each starts with just 5 green glyphs. The autoresearch loop + manual review incrementally grows the green count.

## Integration with Existing Systems

- **Runebender** — already supports markColor in the UI (mark_color_panel.rs)
- **fontspector** — doesn't check markColor (it's metadata, not outline quality)
- **fontc** — ignores markColor (build-only)
- **norad** — reads/writes markColor via `public.markColor` in glyph lib
- **img2ufo** — needs: skip-green logic + set-red-on-trace
- **autoresearch** — needs: color-aware experiment runner

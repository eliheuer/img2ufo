# Reference Sets

9 training datasets for the img2ufo pipeline. Each starts from a scanned type specimen and grows into a fully corrected font through incremental hand-drawing + autoresearch.

## Quick Start

```bash
# Process a specimen (segment + label + build UFO)
./train.sh specimen-001

# Rebuild UFO only (preserves green glyphs)
./train.sh specimen-001 --rebuild

# Rebuild + open in Runebender with background images
./train.sh specimen-001 --open
```

## Color System

Glyphs are color-coded in Runebender to track quality:

| Color | Meaning | On rebuild |
|-------|---------|------------|
| **Green** | Hand-corrected, good | **Kept** — never overwritten |
| Yellow | Almost good, minor fixes needed | Regenerated |
| Orange | Improving, right structure | Regenerated |
| **Red** | Raw pipeline output | Regenerated |

The goal: get everything green. Green glyphs survive regeneration, so you never lose work.

## Setting Up a New Specimen

1. Copy your scan to `references/specimen-NNN/input.png`
2. Edit `config.json` (description, min_area for your scan quality)
3. Run `./train.sh specimen-NNN` — segments the image
4. Label the glyphs:
   - Use Claude Code to read the PNGs and write `unicode-labels.json`
   - Or write `unicode-labels.json` manually
5. Run `./train.sh specimen-NNN --rebuild`
6. Open: `./train.sh specimen-NNN --open`
7. Hand-draw 5 key glyphs: **H, O, n, o, zero** — mark them green

## Bootstrap: 5 Glyphs Per Specimen

These 5 establish the core design parameters:
- **H** — vertical stems, cap height, stroke weight
- **O** — round shapes, overshoot, counter proportions
- **n** — x-height, lowercase stems, arch shape
- **o** — lowercase round, x-height overshoot
- **zero** — numeral proportions, width

Once these 5 are green, autoresearch has enough signal to optimize the rest.

## Structure

```
specimen-001/
  input.png              # Original scan (checked in)
  config.json           # Font metrics + config (checked in)
  unicode-labels.json        # Unicode labels (checked in)
  output.ufo/           # Working font with color-coded glyphs (checked in)
  input-glyphs/                  # Cropped glyph PNGs (gitignored, regenerated)
  comparison/             # Autoresearch diffs (gitignored, regenerated)
```

## Datasets

| # | Description | Status |
|---|-------------|--------|
| specimen-001 | Bold serif (test.png) | Active |
| specimen-002 | (empty) | Needs input.png |
| specimen-003 | (empty) | Needs input.png |
| specimen-004 | (empty) | Needs input.png |
| specimen-005 | (empty) | Needs input.png |
| specimen-006 | (empty) | Needs input.png |
| specimen-007 | (empty) | Needs input.png |
| specimen-008 | (empty) | Needs input.png |
| specimen-009 | (empty) | Needs input.png |

## Autoresearch

The autoresearch loop uses green glyphs as ground truth:

```bash
# Run overnight — compares pipeline output against green references
./autoresearch/run_scan_experiment.sh > run.log 2>&1
```

See `autoresearch/program.md` for the LLM-driven experiment protocol.

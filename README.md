# img2ufo

Convert a AI generated or scanned type specimen image into a Google Fonts-compliant [UFO](https://unifiedfontobject.org/) font source.

This tool is the glue between two other Rust tools:
- [img2glyph](https://github.com/eliheuer/img2glyph) — segments a type specimen into individual glyph PNGs
- [img2bez](https://github.com/eliheuer/img2bez) — traces bitmap glyphs into cubic bezier outlines

img2ufo orchestrates the full pipeline: one image in, one UFO out.

**Agents**: start with [CLAUDE.md](CLAUDE.md) — the validated
image-to-placed-font recipe and the pitfall list. Placement details:
[docs/placement.md](docs/placement.md).

---

## Install

```bash
cargo install --path .
```

You also need [img2glyph](https://github.com/eliheuer/img2glyph) and [fontc](https://github.com/googlefonts/fontc) on PATH:

```bash
cargo install --git https://github.com/eliheuer/img2glyph
cargo install fontc
```

---

## Training workflow

The repo includes 9 training datasets in `references/`. Each starts from a scanned specimen and grows into a corrected font through hand-drawing + autoresearch.

### 1. Process a new specimen

```bash
# Segment the image into individual glyph PNGs
./train.sh specimen-001

# This creates input-glyphs/ and stops — you need to label the glyphs next.
# Use Claude Code to read the PNGs and write unicode-labels.json,
# or create it manually (see references/README.md).
```

### 2. Build the UFO

```bash
# Label + build (after unicode-labels.json exists)
./train.sh specimen-001 --rebuild
```

### 3. Open in Runebender

```bash
# Open the UFO with background images for tracing
./train.sh specimen-001 --open
```

This opens the font in [Runebender](https://github.com/linebender/runebender) with each glyph's source image as a locked background. Trace over the background to correct outlines.

### 4. Hand-draw bootstrap glyphs

Start with 5 key glyphs: **H, O, n, o, zero**. These establish stems, rounds, cap height, x-height, and numeral proportions. Mark each one **green** in Runebender's color panel when you're happy with it, then save.

### 5. Rebuild (greens survive)

```bash
./train.sh specimen-001 --rebuild
```

Green-marked glyphs are **preserved** — only red glyphs get regenerated. This means you never lose hand-drawn work. The workflow is:

1. Draw and mark green → save
2. Rebuild → reds regenerated, greens kept
3. Review → promote improving glyphs to green
4. Repeat until everything is green

**Important:** Mark glyphs green and save in Runebender **before** running `--rebuild`. Uncolored edits will be overwritten.

### Color system

| Color | Meaning | On rebuild |
|-------|---------|------------|
| **Green** | Hand-corrected, good | **Kept** |
| Yellow | Almost good | Regenerated |
| Orange | Improving | Regenerated |
| **Red** | Raw pipeline output | Regenerated |

### train.sh reference

```bash
./train.sh specimen-001              # Segment image into glyph PNGs
./train.sh specimen-001 --rebuild    # Build/rebuild UFO (preserves green)
./train.sh specimen-001 --open       # Open in Runebender (no rebuild)
./train.sh specimen-001 --rebuild --open  # Rebuild then open
```

---

## Direct usage

You can also use img2ufo directly without the training workflow:

```bash
# Segment + label + build in separate steps
img2glyph process input.png --output glyphs/ --min-area 2000
img2glyph label glyphs/manifest.json --assignments labels.json
img2ufo -i input.png -o MyFont-Regular.ufo --glyph-dir glyphs/

# Compile to TTF (adds gasp table, no Python needed)
img2ufo -i input.png -o MyFont-Regular.ufo --glyph-dir glyphs/ --compile

# Open in Runebender with background images
runebender MyFont-Regular.ufo --glyph-images glyphs/
```

---

## How it works

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Specimen     │     │  Glyph PNGs  │     │  UFO font    │
│  image (PNG)  │────▶│  + manifest  │────▶│  source      │
└──────────────┘     └──────────────┘     └──────────────┘
   img2glyph            img2bez              img2ufo
   (segment)           (trace)             (assemble)
```

1. **Segment** — img2glyph binarizes the image, finds connected ink components, crops each glyph with padding, and writes a manifest.

2. **Label** — Unicode codepoints are assigned to each glyph. Use an LLM to read the PNGs and write `unicode-labels.json`, or create it manually.

3. **Trace** — img2bez traces each glyph PNG to cubic bezier curves with smooth-only outlines (on-curve points at extrema, H/V handles).

4. **Assemble** — img2ufo builds a UFO3 font with Google Fonts metadata, GF Latin Core character set (309 codepoints), and power-of-2 grid (1024 UPM).

---

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-i, --input` | required | Input type specimen image |
| `-o, --output` | required | Output UFO directory path |
| `--glyph-dir` | temp dir | Directory for glyph PNGs. If manifest.json exists, segmentation is skipped. |
| `--family-name` | `Untitled` | Font family name |
| `--style-name` | `Regular` | Style name |
| `--upm` | `1024` | Units per em (power-of-2 grid) |
| `--ascender` | `832` | Ascender in font units |
| `--descender` | `-256` | Descender in font units |
| `--cap-height` | `768` | Cap height in font units |
| `--x-height` | `576` | x-height in font units |
| `--accuracy` | `2.0` | Bezier fitting accuracy (lower = tighter) |
| `--grid` | `2` | Coordinate snapping (2 = even integers) |
| `--min-area` | `200` | Minimum glyph area in pixels |
| `--compile` | off | Compile UFO to TTF with fontc |
| `-v, --verbose` | off | Print progress to stderr |

---

## Autoresearch

An overnight experiment loop that systematically improves trace quality. Follows the [Karpathy autoresearch](https://github.com/karpathy/autoresearch) pattern.

```
LLM reads program.md → makes ONE code change → script measures IoU → keep or revert → repeat
```

Green-marked glyphs serve as ground truth. The LLM studies what makes green glyphs good and tries to make the pipeline produce similar output for red glyphs.

```bash
# Run the experiment loop
./autoresearch/run_scan_experiment.sh > run.log 2>&1

# Check results
grep "mean_iou:" run.log
```

See `autoresearch/program.md` for the full protocol.

---

## Training data structure

```
references/
  specimen-001/
    input.png            # Scanned specimen image
    config.json          # Font metrics + segmentation settings
    unicode-labels.json  # Which glyph is which character
    input-glyphs/        # Cropped glyph PNGs (gitignored, regenerated)
    output.ufo/          # The font — green=corrected, red=pipeline
    comparison/          # Autoresearch diffs (gitignored)
```

9 specimen datasets are included. See [`references/README.md`](references/README.md) for setup instructions.

---

## Related tools

- [img2glyph](https://github.com/eliheuer/img2glyph) — glyph segmentation and labeling
- [img2bez](https://github.com/eliheuer/img2bez) — bitmap to bezier tracing
- [Runebender](https://github.com/linebender/runebender) — font editor (background image support for tracing)
- [comfyfont](https://github.com/eliheuer/comfyfont) — ComfyUI font editing nodes

---

## License

MIT

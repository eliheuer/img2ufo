# img2ufo

Convert a scanned type specimen image into a Google Fonts-compliant [UFO](https://unifiedfontobject.org/) font source.

This tool is the glue between two other Rust tools:
- [img2glyph](https://github.com/eliheuer/img2glyph) — segments a type specimen into individual glyph PNGs
- [img2bez](https://github.com/eliheuer/img2bez) — traces bitmap glyphs into cubic bezier outlines

img2ufo orchestrates the full pipeline: one image in, one UFO out.

---

## Install

```bash
cargo install --path .
```

You also need [img2glyph](https://github.com/eliheuer/img2glyph) installed and on PATH for the segmentation step:

```bash
cargo install --git https://github.com/eliheuer/img2glyph
```

---

## Quick start

```bash
# 1. Segment the specimen and save intermediate glyphs
img2glyph process test.png --output glyphs/ --min-area 2000

# 2. Label the glyphs (create assignments.json — see LLM workflow below)
img2glyph label glyphs/manifest.json --assignments glyphs/assignments.json

# 3. Build the UFO
img2ufo -i test.png -o MyFont-Regular.ufo --glyph-dir glyphs/ --min-area 2000
```

If `--glyph-dir` points to a directory that already has a `manifest.json`, segmentation is skipped and only tracing + UFO assembly runs. This lets you iterate on tracing parameters without re-segmenting:

```bash
# Tweak tracing and rebuild (fast — skips segmentation)
img2ufo -i test.png -o MyFont-Regular.ufo --glyph-dir glyphs/ \
    --accuracy 2.0 --smooth-iterations 1 --alphamax 0.80
```

---

## How it works

### Pipeline

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Specimen     │     │  Glyph PNGs  │     │  UFO font    │
│  image (PNG)  │────▶│  + manifest  │────▶│  source      │
│              │     │              │     │              │
└──────────────┘     └──────────────┘     └──────────────┘
   img2glyph            img2bez              img2ufo
   (segment)           (trace)             (assemble)
```

1. **Segment** — img2glyph binarizes the image, finds connected ink components, crops each glyph with padding, and writes a `manifest.json` with bounding boxes and reading order.

2. **Label** — Unicode codepoints and glyph names are assigned to each entry in the manifest. This can be done manually, with a sequence string, or with LLM assistance (see below).

3. **Trace** — img2bez traces each glyph PNG to cubic bezier curves using a Potrace-based pipeline: pixel-edge contour extraction, optimal polygon approximation, corner detection, and curve fitting.

4. **Assemble** — img2ufo builds a UFO3 font source with Google Fonts-required metadata: family/style names, vertical metrics, OS/2 and hhea tables, postscript names, and version string.

### LLM-assisted labeling

img2glyph extracts numbered PNGs in reading order. To assign Unicode codepoints, you need to identify which character each PNG represents. This is where an LLM helps:

1. Open a Claude Code (or similar) session in the glyph directory
2. The LLM reads each glyph PNG and identifies the character
3. It writes `assignments.json` mapping glyph IDs to Unicode codepoints
4. Run `img2glyph label` to apply the assignments

See [img2glyph's LLM workflow docs](https://github.com/eliheuer/img2glyph#llm-workflow) for details.

---

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `-i, --input` | required | Input type specimen image |
| `-o, --output` | required | Output UFO directory path |
| `--glyph-dir` | temp dir | Directory for intermediate glyph PNGs. If it already contains manifest.json, segmentation is skipped. |
| `--family-name` | `Untitled` | Font family name |
| `--style-name` | `Regular` | Style name (Regular, Bold, Italic, etc.) |
| `--upm` | `1000` | Units per em |
| `--ascender` | `800` | Ascender in font units |
| `--descender` | `-200` | Descender in font units |
| `--x-height` | `500` | x-height in font units |
| `--cap-height` | `700` | Cap-height in font units |
| `--accuracy` | `2.0` | Bezier curve-fitting accuracy (lower = tighter fit, more points) |
| `--smooth-iterations` | `1` | Polyline smoothing before curve fitting (0–3) |
| `--alphamax` | `0.80` | Corner detection threshold (lower = more corners) |
| `--grid` | `0` | Coordinate snapping grid (0 = off) |
| `--min-area` | `200` | Minimum glyph area in pixels (raise to filter scan noise) |
| `--max-area` | `50000` | Maximum glyph area in pixels |
| `-v, --verbose` | off | Print progress to stderr |

---

## Autoresearch

img2ufo includes an autoresearch framework for systematically improving trace quality on scanned specimens. It follows the [Karpathy autoresearch](https://github.com/karpathy/autoresearch) pattern: an LLM proposes code changes, a script measures the result, and changes are kept or discarded based on a single metric.

### How it works

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────┐
│ LLM reads   │     │ LLM makes   │     │ Script runs │     │ Keep or │
│ program.md  │────▶│ ONE change  │────▶│ pipeline +  │────▶│ revert  │
│             │     │ to code     │     │ measures IoU│     │         │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────┘
       ▲                                                         │
       └─────────────────────────────────────────────────────────┘
                              repeat forever
```

1. **`program.md`** — instructions the LLM reads. Describes what to optimize (raster IoU), what code to change (img2bez, img2glyph, img2ufo), and the keep/discard protocol.

2. **`run_scan_experiment.sh`** — traces input glyph PNGs with img2bez, compares each against a hand-corrected reference `.glif`, and reports mean raster IoU %.

3. **Reference sets** — each set has cropped glyph PNGs (input) and a hand-corrected UFO (ground truth). The LLM can't see these but the script measures against them.

### Reference sets

```
references/
  specimen-001/
    source.png              # Original full-page scan
    metadata.json           # Font metrics + config
    assignments.json        # Unicode labels
    input/                  # Cropped glyph PNGs (gitignored, from img2glyph)
    expected.ufo/           # Hand-corrected outlines (ground truth)
    pipeline-output.ufo/    # Latest pipeline output (gitignored)
    comparison/             # Visual diffs (gitignored, generated)
    results.tsv             # Experiment history
```

To create a reference set:

1. Segment and label a specimen (the `input/` directory)
2. Run the pipeline to get `pipeline-output.ufo`
3. Open in a font editor, correct the outlines, save as `expected.ufo`

Even 10 corrected glyphs is enough to start the loop. See [`references/README.md`](references/README.md) for details.

### Running the loop

Point any LLM coding agent at `autoresearch/program.md`:

```bash
# Run one experiment with specific parameters
./autoresearch/run_scan_experiment.sh --alphamax 0.9 --accuracy 6.0

# Or let an LLM drive it overnight (~100 experiments)
# The LLM reads program.md, makes changes, runs the script, keeps or reverts
```

The script outputs parseable results:
```
mean_iou: 72.45%
mean_score: 0.634
glyphs_ok: 26
glyphs_failed: 0
```

### What the LLM can change

Code in all three repos (img2bez, img2glyph, img2ufo):
- Thresholding and hole filling (`img2bez/src/bitmap.rs`)
- Curve fitting constants (`img2bez/src/vectorize/curve.rs`)
- Segmentation and masking (`img2glyph/src/segment.rs`)
- Pipeline config and defaults (`img2ufo/src/ufo_builder.rs`)

---

## Project structure

```
img2ufo/
├── src/
│   ├── main.rs            # CLI entry point
│   ├── pipeline.rs        # Pipeline orchestration
│   ├── ufo_builder.rs     # Tracing + UFO assembly
│   └── manifest.rs        # img2glyph manifest deserialization
├── autoresearch/
│   ├── program.md         # LLM instructions for the experiment loop
│   └── run_scan_experiment.sh
├── references/
│   ├── README.md
│   └── specimen-001/      # First reference set
├── context/
│   └── scan-quality-strategy.md
├── test.png               # Test specimen image
└── Cargo.toml
```

---

## Related tools

- [img2glyph](https://github.com/eliheuer/img2glyph) — glyph segmentation and labeling
- [img2bez](https://github.com/eliheuer/img2bez) — bitmap to bezier tracing
- [comfyfont](https://github.com/eliheuer/comfyfont) — ComfyUI font editing nodes

---

## License

MIT

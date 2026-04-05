#!/usr/bin/env bash
# train.sh — Process a specimen through the full pipeline.
#
# Usage:
#   ./train.sh specimen-001              # segment + label + build UFO
#   ./train.sh specimen-001 --rebuild    # rebuild UFO only (skip segmentation)
#   ./train.sh specimen-001 --open       # rebuild + open in Runebender
#
# Green glyphs are preserved on rebuild — only red/uncolored glyphs get regenerated.
#
# Setup for a new specimen:
#   1. Copy your scan to references/specimen-NNN/input.png
#   2. Edit references/specimen-NNN/config.json (description, min_area, etc.)
#   3. Run: ./train.sh specimen-NNN
#   4. Use Claude Code to create unicode-labels.json (LLM reads glyph PNGs)
#   5. Run: ./train.sh specimen-NNN --rebuild
#   6. Open in Runebender: ./train.sh specimen-NNN --open
#   7. Hand-draw 5 key glyphs (H, O, n, o, zero), mark them green

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# ── Parse args ────────────────────────────────────────────────────────────────

SPECIMEN="${1:?Usage: ./train.sh <specimen-name> [--rebuild] [--open]}"
REBUILD=false
OPEN=false

shift
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rebuild) REBUILD=true ;;
        --open)    OPEN=true; REBUILD=true ;;
        *) echo "Unknown flag: $1"; exit 1 ;;
    esac
    shift
done

# ── Resolve paths ─────────────────────────────────────────────────────────────

REF_DIR="references/$SPECIMEN"
if [ ! -d "$REF_DIR" ]; then
    echo "ERROR: $REF_DIR does not exist"
    echo "Available specimens:"
    ls references/ | grep specimen
    exit 1
fi

SOURCE="$REF_DIR/input.png"
INPUT_DIR="$REF_DIR/input"
ASSIGNMENTS="$REF_DIR/unicode-labels.json"
METADATA="$REF_DIR/config.json"
UFO="$REF_DIR/output.ufo"

# Read font metrics from metadata.
FAMILY_NAME=$(python3 -c "import json; print(json.load(open('$METADATA')).get('description','Untitled') or 'Untitled')")
MIN_AREA=$(python3 -c "import json; print(json.load(open('$METADATA'))['segmentation']['min_area'])")

# ── Step 1: Segment (skip if --rebuild or input-glyphs/ already has manifest) ────────

if [ "$REBUILD" = false ]; then
    if [ ! -f "$SOURCE" ]; then
        echo "ERROR: No source image at $SOURCE"
        echo "Copy your scan there and try again."
        exit 1
    fi

    echo "=== Segmenting $SOURCE ==="
    img2glyph process "$SOURCE" --output "$INPUT_DIR" --min-area "$MIN_AREA"

    echo ""
    echo "=== Segmentation complete ==="
    echo "Glyphs extracted to: $INPUT_DIR"
    echo ""

    if [ ! -f "$ASSIGNMENTS" ]; then
        echo "Next step: create $ASSIGNMENTS"
        echo "  Option A: Use Claude Code to read the glyph PNGs and write assignments"
        echo "  Option B: Create manually (see references/README.md)"
        echo ""
        echo "Then run: ./train.sh $SPECIMEN --rebuild"
        exit 0
    fi
fi

# ── Step 2: Label (if assignments exist but glyphs aren't labeled) ────────────

if [ -f "$ASSIGNMENTS" ] && [ -f "$INPUT_DIR/manifest.json" ]; then
    # Check if manifest has labels
    HAS_LABELS=$(python3 -c "
import json
m = json.load(open('$INPUT_DIR/manifest.json'))
labeled = sum(1 for g in m['glyphs'] if g.get('glyph_name'))
print('yes' if labeled > 0 else 'no')
")
    if [ "$HAS_LABELS" = "no" ]; then
        echo "=== Labeling glyphs ==="
        img2glyph label "$INPUT_DIR/manifest.json" --assignments "$ASSIGNMENTS"
    fi
fi

# ── Step 3: Build UFO ────────────────────────────────────────────────────────

if [ ! -f "$INPUT_DIR/manifest.json" ]; then
    echo "ERROR: No manifest.json in $INPUT_DIR"
    echo "Run without --rebuild first: ./train.sh $SPECIMEN"
    exit 1
fi

echo "=== Building UFO ==="
cargo run --quiet -- \
    -i "${SOURCE:-/dev/null}" \
    -o "$UFO" \
    --glyph-dir "$INPUT_DIR" \
    --min-area "$MIN_AREA" \
    --family-name "$FAMILY_NAME"

# Count colors
python3 -c "
import os, xml.etree.ElementTree as ET

ufo = '$UFO'
glyphs_dir = f'{ufo}/glyphs'
colors = {'green': 0, 'yellow': 0, 'orange': 0, 'red': 0, 'none': 0}

for f in sorted(os.listdir(glyphs_dir)):
    if not f.endswith('.glif'): continue
    tree = ET.parse(os.path.join(glyphs_dir, f))
    lib = tree.find('.//lib')
    mc = None
    if lib is not None:
        d = lib.find('dict')
        if d is not None:
            keys = [e.text for e in d.findall('key')]
            vals = [e.text for e in d.findall('string')]
            kv = dict(zip(keys, vals))
            mc = kv.get('public.markColor', '')

    if mc and mc.startswith('0,1,0'):       colors['green'] += 1
    elif mc and mc.startswith('1,1,0'):     colors['yellow'] += 1
    elif mc and mc.startswith('1,0.5,0'):   colors['orange'] += 1
    elif mc and mc.startswith('1,0,0'):     colors['red'] += 1
    else:                                   colors['none'] += 1

total = sum(colors.values())
print()
print(f'=== {ufo} ===')
print(f'  Total glyphs: {total}')
for color, count in colors.items():
    if count > 0:
        bar = '█' * count
        print(f'  {color:8} {count:3}  {bar}')
print()
"

# ── Step 4: Open in Runebender (if --open) ────────────────────────────────────

if [ "$OPEN" = true ]; then
    echo "=== Opening in Runebender ==="
    runebender "$UFO" --glyph-images "$INPUT_DIR" &
fi

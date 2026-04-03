#!/usr/bin/env bash
# run_scan_experiment.sh — autoresearch experiment runner for scanned specimens
#
# Traces pre-existing glyph PNGs with img2bez, compares against hand-corrected
# reference .glif files, and reports raster IoU + vector quality score.
#
# Usage:
#   ./autoresearch/run_scan_experiment.sh [extra img2bez flags...]
#
# Example:
#   ./autoresearch/run_scan_experiment.sh --alphamax 0.9 --accuracy 6.0
#   REFERENCE_SET=references/specimen-002 ./autoresearch/run_scan_experiment.sh
#
# Outputs (parseable by agent):
#   mean_iou: XX.XX%
#   mean_score: 0.XXX
#   glyphs_ok: N
#   glyphs_failed: N
#
# Exit codes:
#   0 = success
#   1 = build failed, no reference set, or no glyphs evaluated

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Configuration ─────────────────────────────────────────────────────────────

# Reference set directory. Must contain input/*.png and expected.ufo/
REFERENCE_SET="${REFERENCE_SET:-$REPO_DIR/references/specimen-001}"

# Path to img2bez repo (sibling directory by default)
IMG2BEZ_DIR="${IMG2BEZ_DIR:-$REPO_DIR/../img2bez}"

# Extra img2bez flags from command line
EXTRA_PARAMS=()
if [ "$#" -gt 0 ]; then
    EXTRA_PARAMS=("$@")
fi

# ── Validate prerequisites ────────────────────────────────────────────────────

INPUT_DIR="$REFERENCE_SET/input"
EXPECTED_UFO="$REFERENCE_SET/expected.ufo"
METADATA="$REFERENCE_SET/metadata.json"
WORK_DIR="$REFERENCE_SET/comparison"

if [ ! -d "$INPUT_DIR" ]; then
    echo "ERROR: Input directory not found: $INPUT_DIR"
    exit 1
fi

if [ ! -d "$EXPECTED_UFO" ]; then
    echo "ERROR: expected.ufo not found at $EXPECTED_UFO"
    echo ""
    echo "To create it:"
    echo "  1. Open $REFERENCE_SET/pipeline-output.ufo in a font editor"
    echo "  2. Correct the outlines to match source.png"
    echo "  3. Save as $EXPECTED_UFO"
    exit 1
fi

if [ ! -f "$METADATA" ]; then
    echo "ERROR: metadata.json not found at $METADATA"
    exit 1
fi

# ── Build img2bez ─────────────────────────────────────────────────────────────

echo "Building img2bez..."
cd "$IMG2BEZ_DIR"
if ! cargo build --release 2>&1; then
    echo "ERROR: Build failed"
    exit 1
fi
BINARY="$IMG2BEZ_DIR/target/release/img2bez"
echo "Build OK"
echo ""

# ── Read font metrics ─────────────────────────────────────────────────────────

read -r TARGET_HEIGHT Y_OFFSET < <(python3 -c "
import json, sys
m = json.load(open('$METADATA'))['font_metrics']
asc = m.get('ascender', 800)
desc = m.get('descender', -200)
print(f'{asc - desc} {desc}')
")

echo "Reference set: $(basename "$REFERENCE_SET")"
echo "Target height: $TARGET_HEIGHT  Y offset: $Y_OFFSET"

# ── Discover reference glyphs ─────────────────────────────────────────────────

GLYPHS_DIR="$EXPECTED_UFO/glyphs"
CONTENTS="$GLYPHS_DIR/contents.plist"

if [ ! -f "$CONTENTS" ]; then
    echo "ERROR: No contents.plist in $GLYPHS_DIR"
    exit 1
fi

# Build list of glyphs that have BOTH an input PNG and a reference .glif
GLYPH_PAIRS=()
while IFS=$'\t' read -r glyph_name glif_file; do
    png="$INPUT_DIR/${glyph_name}.png"
    glif="$GLYPHS_DIR/$glif_file"
    if [ -f "$png" ] && [ -f "$glif" ]; then
        GLYPH_PAIRS+=("$glyph_name")
    fi
done < <(python3 - "$CONTENTS" <<'PYEOF'
import plistlib, sys
with open(sys.argv[1], "rb") as f:
    contents = plistlib.load(f)
for name, glif in sorted(contents.items()):
    print(f"{name}\t{glif}")
PYEOF
)

echo "Glyphs to evaluate: ${#GLYPH_PAIRS[@]}"
if [ "${#GLYPH_PAIRS[@]}" -gt 0 ]; then
    echo "  ${GLYPH_PAIRS[*]}"
fi
echo ""

if [ "${#GLYPH_PAIRS[@]}" -eq 0 ]; then
    echo "ERROR: No matching glyph pairs found (need both input PNG and reference .glif)"
    exit 1
fi

# ── Set up work directory ─────────────────────────────────────────────────────

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

# Create minimal output UFO
OUTPUT_UFO="$WORK_DIR/output.ufo"
mkdir -p "$OUTPUT_UFO/glyphs"
cat > "$OUTPUT_UFO/metainfo.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>creator</key><string>img2ufo autoresearch</string>
  <key>formatVersion</key><integer>3</integer>
</dict></plist>
PLIST
python3 -c "
import plistlib
with open('$OUTPUT_UFO/glyphs/contents.plist', 'wb') as f:
    plistlib.dump({}, f)
"

# ── Run per-glyph experiments ─────────────────────────────────────────────────

total_iou=0
total_score=0
count_ok=0
count_fail=0

for glyph_name in "${GLYPH_PAIRS[@]}"; do
    png_path="$INPUT_DIR/${glyph_name}.png"

    # Resolve .glif path
    glif_path=$(python3 - "$CONTENTS" "$glyph_name" "$GLYPHS_DIR" <<'PYEOF'
import plistlib, os, sys
with open(sys.argv[1], "rb") as f:
    contents = plistlib.load(f)
print(os.path.join(sys.argv[3], contents[sys.argv[2]]))
PYEOF
    )

    # Safe filename (avoid case collisions on macOS)
    file_key=$(python3 -c "
c = '$glyph_name'
print(f'uni{ord(c):04X}' if len(c) == 1 else c)
")
    log_path="$WORK_DIR/${file_key}.log"

    # Trace and compare
    "$BINARY" \
        --input "$png_path" \
        --output "$OUTPUT_UFO" \
        --name "$glyph_name" \
        --target-height "$TARGET_HEIGHT" \
        --y-offset "$Y_OFFSET" \
        --reference "$glif_path" \
        ${EXTRA_PARAMS[@]+"${EXTRA_PARAMS[@]}"} \
        > "$log_path" 2>&1 || true

    # Parse metrics
    iou=$(grep "Raster IoU" "$log_path" \
        | grep -oE '[0-9]+\.[0-9]+' | head -1 || echo "")
    score=$(grep "Overall" "$log_path" \
        | grep -oE '[0-9]+\.[0-9]+' | tail -1 || echo "")

    if [ -n "$iou" ] && [ -n "$score" ]; then
        printf "  %-12s  IoU=%6.2f%%  score=%.3f\n" "$glyph_name" "$iou" "$score"
        total_iou=$(python3 -c "print($total_iou + $iou)")
        total_score=$(python3 -c "print($total_score + $score)")
        count_ok=$((count_ok + 1))
    else
        echo "  FAIL $glyph_name (no metrics — see $log_path)"
        count_fail=$((count_fail + 1))
    fi
done

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
if [ "$count_ok" -gt 0 ]; then
    mean_iou=$(python3 -c "print(f'{$total_iou / $count_ok:.2f}')")
    mean_score=$(python3 -c "print(f'{$total_score / $count_ok:.3f}')")
    echo "mean_iou: ${mean_iou}%"
    echo "mean_score: ${mean_score}"
    echo "glyphs_ok: ${count_ok}"
    echo "glyphs_failed: ${count_fail}"
else
    echo "ERROR: No glyphs evaluated successfully"
    echo "glyphs_ok: 0"
    echo "glyphs_failed: ${count_fail}"
    exit 1
fi

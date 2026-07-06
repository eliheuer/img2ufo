# Glyph-completion harness: finishing an img2ufo font with an agent

2026-07-05. The contract between img2ufo's output and a downstream
glyph-completion agent (Codex desktop + OpenAI image API, Claude, or a
human designer). img2ufo composes everything composable; the agent's job
is ONLY to supply the ink the specimen never provided. The deterministic
pipeline does everything else — tracing, anchoring, composition, metrics,
QA — so the agent never touches font engineering.

## What the pipeline already does (no agent needed)

After tracing, the auto-composition stage (`src/composites.rs`):

1. Adds UFO anchors to bases (`top`, `bottom`, `ogonek`, `topright`) and
   marks (`_top`, `_bottom`, `_ogonek`, `_topright`). Hand-placed anchors
   are never overwritten.
2. Derives ink-free glyphs: `dotlessi`/`dotlessj` (tittle removal),
   spacing accent <-> combining mark aliases (component references).
3. Builds every GF Latin Core composite whose base + mark exist, as
   components positioned by anchor arithmetic (`~140` glyphs when a full
   mark set is present).
4. Emits the worklist: `<Family>-<Style>-completion.json` next to the UFO.

Nothing is faked: a composite whose mark is missing stays missing and is
reported, per docs/gf-compliance-checklist.md ("missing glyphs must be
designed, not synthesized").

## The worklist file

```jsonc
{
  "family": "...",
  "glyphset": "GF_Latin_Core",
  "coverage_traced": {"covered": 80, "total": 319},
  "coverage_after_composition": {"covered": 82, "total": 319},
  "built_composites": [{"name": "Aacute", "base": "A", "mark": "acutecomb", "anchor": "top"}],
  "derived_glyphs":  [{"name": "dotlessi", "from": "i", "method": "tittle removed"}],
  "missing_marks":   [{"name": "acutecomb", "codepoint": "U+0301", "unlocks": ["Aacute", "..."]}],
  "missing_atomic":  [{"name": "quotedbl", "codepoint": "U+0022"}]
}
```

- `missing_marks` is sorted by unlock count — **the agent's priority
  queue**. Drawing `acutecomb` on a bare Letraset sheet unlocks ~27
  glyphs; `caroncomb.alt` unlocks the dcaron/lcaron/Lcaron/tcaron family.
  Marks without a codepoint (`caroncomb.alt`, `commaturnedabovecomb`)
  are component-only glyphs, still required for full coverage and for
  fontspector's `case_mapping` check (an uppercase composite without its
  lowercase counterpart is a FAIL — e.g. Dcaron traced but dcaron needs
  `caroncomb.alt`).
- `missing_atomic` is everything that must be drawn outright:
  punctuation, symbols, currency, and the non-composable letters
  (AE OE Eth Thorn Dcroat Hbar Lslash Germandbls ordfeminine ...).

## Mark-color protocol (rebuild safety)

| color  | meaning                                | on rebuild |
|--------|----------------------------------------|------------|
| green  | hand-corrected / agent-finalized       | preserved  |
| red    | traced pipeline output                 | regenerated|
| yellow | auto-composite (components)            | regenerated|
| orange | derived ink (dotless, mark aliases)    | regenerated|

An agent that finishes a glyph must set `public.markColor` to green
(`0.3,0.7,0.3,1`) or the next rebuild replaces it.

## Route A (preferred): the agent supplies raster crops

The agent never edits the UFO. It supplies new labeled crops in the
glyph dir, exactly like the specimen did, and re-runs the pipeline:

1. Read `<stem>-completion.json`; pick the highest-unlock item.
2. Generate a grayscale PNG of the glyph **in the specimen's style**
   (image model with reference images):
   - reference: the existing crops in the glyph dir (or contact.png);
     include 6-10 stylistically load-bearing ones (o n H O e s comma).
   - black ink on white, one glyph, no anti-aliasing artifacts beyond
     what the reference crops show, generous margins.
   - scale: match the sheet. Caps are ~cap-height-px tall (see any
     uppercase crop); a top mark is roughly 1/4-1/3 of that; ascenders/
     descenders proportional to the d/p crops. The PNG's resolution is
     independent of the manifest bbox (placement uses the bbox), so
     render large (300-600 px) for clean tracing.
3. Append a manifest entry (`glyph_NNNN`, bbox roughly positioned in an
   existing row — for marks the vertical position barely matters, anchor
   arithmetic normalizes it) and a labels entry (`"U+0301"` etc.;
   unencoded marks like `caroncomb.alt` use `{"glyph_name":
   "caroncomb.alt"}` with no unicode).
4. Re-run img2ufo with the same arguments. Trace, anchors, composition,
   coverage, fontspector all update. Green glyphs survive.
5. Loop until `missing_marks` and `missing_atomic` are empty and
   fontspector's `googlefonts/glyph_coverage` + `glyphsets/shape_languages`
   pass. Judge each new glyph against the originals (img2bez's judge
   gate applies to traced crops automatically).

This keeps the AI on the perception side of the img2bez principle:
generative models assist perception, deterministic code decides geometry.

## Route B: the agent edits vectors directly

For an agent that can write UFO `.glif` XML (or a human in Runebender
via `runebender-serve`): draw the glyph, keep integer coordinates,
cubic curves, correct direction (counter-clockwise outer), points at
extrema — img2bez conventions — then mark it green. `fontc` + fontspector
still gate the result. Anchors may be hand-adjusted; composition respects
existing anchors on rebuild.

## What the agent must never do

- Overwrite or "improve" green glyphs.
- Invent vertical metrics, names, or fontinfo — the pipeline owns them.
- Add glyphs by copying/transforming other glyphs' ink to fake a mark
  (rotating a comma into a turned comma is a design decision; draw it).
- Commit `fonts/` build artifacts (gitignored in emitted repos).

## Remaining human/legal gates (from the compliance checklist)

Automation stops where the GF guide demands a human: family name,
copyright holder + upstream URL, the license status of the source
specimen, category/designer metadata, and the google/fonts submission
issue. See docs/gf-compliance-checklist.md.

# The Mekorot output style — smooth × powers-of-two grid

2026-07-11. Eli's design decision (sketch reviewed in session): the
starting representation for the Letraset scan project, and eventually
the native encoding of the Mekorot model. Named for מקורות — "sources."

## The project this serves

**Mekorot the font**: Eli's unfinished Arabic/Hebrew family, missing a
Latin in the same style. The plan (starting week of 2026-07-13): trace
a Letraset Latin specimen in this style and merge three sources into
one cohesive tri-script family — the first end-to-end test of the idea.

- Hebrew: https://github.com/eliheuer/mekorot — local
  `/Users/eli/GH/repos/mekorot/sources/` (Regular + ExtraBold + italics,
  UPM 1024). Measured: alef is 22/22 smooth with 19/22 tangents already
  axis-aligned, but only ~2% grid-aligned — conversion is snapping plus
  a per-glyph decision on the few diagonal tangents.
- Arabic: https://github.com/eliheuer/open-gate-naskh — local
  `/Users/eli/GH/repos/open-gate-naskh/sources/` (Regular, UPM 1024,
  heavy use of components; needs a structural audit before conversion).
- Latin: does not exist in the style yet (the current Latin in the
  Hebrew repo is cornered straight-line construction — replace, don't
  patch). Source: a Letraset specimen of Eli's choosing from
  `/Users/eli/Desktop/letraset/`, traced in this style.

The style spec below is therefore also the HARMONIZATION CONTRACT
across the three scripts: same grid, same invariant, same point
grammar, one family.

## The invariant

Every on-curve point is **smooth** with an **exactly horizontal or
vertical tangent**, and every coordinate lies on the powers-of-two grid.

These compose without tension because the tangent constraint makes
collinearity structural: a vertical-tangent point's two handles share
its x (a horizontal-tangent point's share its y), so the only free
quantity per handle is its LENGTH — a single grid scalar. There is no
corner vocabulary, no corner-transition logic, no smooth-vs-grid
reconciliation. Nothing an editor can "fix" on touch.

Canonical per-point form:

    point = (x, y, tangent ∈ {H, V}, len_in, len_out)   — 5 grid numbers

(Standard cubic UFO outlines are recovered deterministically: handles
extend along the tangent by their lengths.)

## Consequences (all load-bearing)

- **Quarter-turn arcs.** Between consecutive points the curve turns
  monotonically through exactly 90°. Every contour is a chain of
  quarter-turn arcs — the "stone smoothed in a river / sanded wood"
  effect is a theorem of the representation, not a post-process.
- **Canonical structure.** Points exist at curve extrema, only at
  extrema, all smooth: the same shape always yields the same point
  structure. This manufactures the skeleton-consistency property that
  makes glyph ML tractable (the reason Chinese font generation works).
- **Compact and fittable.** Per point the tracer optimizes 2 grid
  scalars (the handle lengths) instead of 4 continuous coordinates.
  Token sequences shrink accordingly; the outline grammar has one
  point type.
- **Geometric primitives are native.** Circle / superellipse / blob =
  4 points. The style traces geometric shapes as naturally as letters.

## Scope boundary

True diagonals cannot exist: straight diagonal strokes (A, V, X)
render as gentle S-flows between H/V extrema. For rounded/organic
display faces this IS the aesthetic; for cornered faces it is a
reinterpretation. Style selection is a per-sheet human decision —
Virtua's corners-and-straights style and the Mekorot smooth style are
siblings, chosen per source.

## Model plan

One architecture, style token in the vocabulary, shared corpus — split
Mekorot into separate weights only if measured interference justifies
it (small-data transfer argues for sharing until proven otherwise).
Data quality uses the per-glyph mark-color scale (green human-approved
/ yellow trusted-not-graded / orange machine draft / red garbage).

## Implementation order

1. img2bez output style `mekorot`: extrema detection -> H/V smooth
   points on grid -> fit handle lengths (2 scalars/point) -> UFO.
2. Letraset pilot (~24 sheets on the Desktop): segment -> char ID ->
   upscale (gated by cycle-consistency vs the ORIGINAL scan) -> trace
   in mekorot style -> per-glyph mark colors -> UFO.
3. Corpus: re-trace suitable OFL rounded faces in the style for scale;
   scans stay the gold-provenance source.
4. Mekorot the model, once the corpus exists (style token first;
   separate weights only if earned).

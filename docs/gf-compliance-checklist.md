# Google Fonts compliance checklist for img2ufo

Requirements for a font source produced by the automated specimen→UFO
pipeline, distilled from the GF Guide (https://googlefonts.github.io/gf-guide/)
on 2026-07-05, plus fontspector integration notes.

Tags: **[AUTO]** img2ufo can fully automate · **[AUTO+DEFAULT]** automatable
with an overridable default · **[HUMAN]** requires a human decision.

## 1. Vertical metrics
Source: https://googlefonts.github.io/gf-guide/metrics.html

- [AUTO] Set OS/2 fsSelection bit 7 (USE_TYPO_METRICS).
- [AUTO] typoLineGap = 0 and hhea.lineGap = 0.
- [AUTO] hhea must equal typo: hhea.ascender = typoAscender,
  hhea.descender = typoDescender.
- [AUTO] winAscent / winDescent = the family-wide tallest yMax / deepest
  yMin bounding-box values (prevents clipping; win values are per-family,
  not per-font).
- [AUTO+DEFAULT] |typoAscender| + |typoDescender| + typoLineGap should be
  20–30% greater than UPM (UPM 1000 → ~1200).
- [AUTO+DEFAULT] typoAscender should clear the tallest stacked accent
  (guide reference: U+1EAE Abreveacute, measured across all weights);
  typoAscender − capHeight ≈ |typoDescender| to vertically center caps.
- [AUTO] Every font in the family shares identical vertical metrics.
- [HUMAN] Upgrades of already-published families: metrics must not change;
  line height must visually match the prior release (human confirms which
  release governs).

## 2. Name table / OS/2 / fontinfo.plist
Source: https://googlefonts.github.io/gf-guide/requirements.html

- [AUTO] fsType = 0 (installable embedding). UFO: `openTypeOS2Type = []`.
- [HUMAN] Family name choice — constraints are checkable [AUTO]: no
  camelCase, no abbreviations, no all-caps; ASCII letters/digits/spaces
  only; family+style ≤ 32 chars; must not contain "color/colr/colored".
- [AUTO] Copyright (name ID 0 / UFO `copyright`), exact format:
  `Copyright { year } The { family } Project Authors ({ git_url })`.
  Year + git URL are [HUMAN] inputs; string assembly is [AUTO].
- [AUTO] Name ID 13 (UFO `openTypeNameLicense`), exact string:
  `This Font Software is licensed under the SIL Open Font License,
  Version 1.1. This license is available with a FAQ at:
  https://openfontlicense.org`
- [AUTO] Name ID 14 (UFO `openTypeNameLicenseURL`):
  `https://openfontlicense.org`
- [AUTO] Version format `MAJOR.MINORPATCH`, e.g. 1.230 → 2.000 breaking
  change, → 1.330 new charset, → 1.240 new glyphs, → 1.231 metadata fix.
  UFO: `versionMajor`/`versionMinor`. Bump semantics [AUTO+DEFAULT].
- [AUTO] RIBBI style linking: name ID 2 only Regular / Italic / Bold /
  Bold Italic; other style particles move to ID 1, real names in 16/17.
- [AUTO+DEFAULT] OS/2 achVendID (UFO `openTypeOS2VendorID`): no mandated
  value, but set a real vendor tag (fontspector warns on unknown/empty).
- [AUTO] usWeightClass matches the style: 100–900 in steps of 100.
- [AUTO+DEFAULT] Proportional default figures + `tnum` feature; monospace
  fonts need post.isFixedPitch = 1 and matching panose bProportion.

## 3. Glyphsets
Sources: https://googlefonts.github.io/gf-guide/onboarding.html,
https://github.com/googlefonts/glyphsets

- [AUTO] Minimum for new fonts: **GF Latin Core**, ~324 glyphs
  (GF_Latin_Kernel's 116 + 208 more): ASCII, standard punctuation,
  common precomposed diacritics, basic currency/symbols.
- [AUTO] Machine-readable definitions: nice-names list at
  `googlefonts/glyphsets/blob/main/data/results/txt/nice-names/GF_Latin_Core.txt`;
  YAML definitions in `/Lib/glyphsets/definitions`; pip package
  `glyphsets` (also emits .nam, production-name txt, .plist filters).
  img2ufo should verify coverage programmatically against this.
- [HUMAN] Whether the traced specimen actually yields all Latin Core
  glyphs — missing glyphs must be designed, not synthesized. Choosing
  larger sets (GF_Latin_Plus, other scripts) is a design decision.

## 4. Outline quality
Source: https://googlefonts.github.io/gf-guide/outlines.html

- [AUTO] No open contours.
- [AUTO] Correct direction: cubic/PostScript sources counter-clockwise
  outer / clockwise counters (TrueType binaries are flipped by the
  compiler; source needs internal consistency).
- [AUTO] Points at extrema.
- [AUTO] Integer coordinates only (explicit GF requirement, on-curve and
  off-curve points).
- [AUTO] Overlaps: **keep in sources and variable fonts**; removed when
  generating static binaries. No self-intersecting outlines in shipped
  statics; decompose at least one component where components overlap.
- [AUTO] No kinks at smooth joins across interpolation, no stray points,
  no vestigial 1–2-unit segments, no near-misses 1–2 units off an
  alignment zone (guide gives no hard thresholds; fontspector does).
- [AUTO] One curve flavor per source (all-cubic UFO → quadratic at TTF
  build; never mixed).
- [AUTO] Variable fonts: point-compatible contours across masters;
  interpolation-safe curve segments.

Note: this section is img2ufo's home turf — img2bez already enforces
closed contours, extrema, integer coords, direction, and no-vestigial-
segment structurally at trace time.

## 5. Static vs variable requirements
Sources: https://googlefonts.github.io/gf-guide/statics.html,
https://googlefonts.github.io/gf-guide/variable.html

- [AUTO+DEFAULT] VF is the practical default: if a VF exists and statics
  are merely autohinted, only the VF is onboarded (both only when
  statics are manually hinted).
- [AUTO] Regular (400) must exist; wght axis range must include 400.
  Single-style family → "One" suffix (`FamilyNameOne-Regular.ttf`).
- [AUTO] Up to 18 static styles: 9 weights 100–900 + matching italics.
- [AUTO] fvar named instances: only weight and italic particles
  (Thin…Black + Italics), coordinates consistent with usWeightClass.
- [AUTO] STAT table mandatory in VFs; wght and ital axes ordered last;
  STAT entries for every fvar instance; user-space values.
- [AUTO] No `ital` axis inside one VF: separate `Family[axes].ttf` and
  `Family-Italic[axes].ttf`. Filename axis tags: custom uppercase first
  then registered lowercase, each group alphabetical. Custom axes must
  exist in the GF Axis Registry or the font can't ship.
- [AUTO] italicAngle 0 for uprights, negative for right-leaning italics.
  Measuring the angle is [AUTO]; declaring a family Italic is [HUMAN].

## 6. License (OFL)
Source: https://googlefonts.github.io/gf-guide/license-file.html

- [AUTO] OFL.txt uses the standardized GF template; only the first line
  varies — the copyright string, byte-identical to name ID 0.
- [AUTO+DEFAULT] **No Reserved Font Names.** GF's template omits the RFN
  mention and GF strongly discourages RFNs (they serve modified/subset
  files). RFN exceptions (revivals) need written permission via
  fonts@google.com + a fontbakery/fontspector exception — [HUMAN] path.
- [AUTO] Name IDs 13/14 exactly as in section 2.
- [HUMAN] Copyright holder identity, year, and confirmation the whole
  project is wholly OFL-1.1 — **including the input specimen image's
  license. img2ufo cannot decide whether a traced specimen is legally
  OFL-able; this is the single biggest human/legal gate for the
  pipeline.**

## 7. Upstream repo conventions
Source: https://googlefonts.github.io/gf-guide/upstream.html

- [AUTO] Layout: `sources/` (UFO/designspace source of truth + build
  config), `fonts/` (`ttf/`, `otf/`, `webfonts/`, `variable/`),
  `documentation/` (optional `article/`, `social-assets/`).
- [AUTO] Required files: `OFL.txt` (first line = copyright string),
  `README.md` (description, images, build instructions), `AUTHORS.txt`,
  `CONTRIBUTORS.txt`, `requirements.txt`, `.gitignore`.
- [AUTO] One-command build: `config.yaml` for gftools-builder
  (`gftools builder sources/config.yaml`, fontmake underneath) or a
  `build.sh`.
- [AUTO+DEFAULT] Base on googlefonts-project-template
  (github.com/googlefonts/googlefonts-project-template) — recommended,
  not mandatory; ships build + QA GitHub Actions. Source filenames:
  `FontFamily.ext` / `FontFamily-Italic.ext`.
- [HUMAN] Repo must be public, releases tagged, owned by a maintainer
  (upgrades from forks are not accepted).
- Accepted source formats for new fonts: UFO, .glyphs, .glyphspackage,
  fontforge — img2ufo's UFO output is directly acceptable [AUTO].
  Shipped binaries are TTF only.

## 8. Onboarding process & QA gate
Sources: https://googlefonts.github.io/gf-guide/onboarding.html,
/making-pr.html, /metadata.html, /qa.html, /article.html

- [HUMAN] Entry point: file an issue on the google/fonts tracker first —
  "If your font isn't submitted through an issue first, your PR may
  never be merged." Third parties can PR, but the GF team (via
  `gftools packager`) does most onboarding; acceptance is curatorial
  (original design or legitimate public-domain revival).
- [AUTO] PR payload in `ofl/{familyname}/`: TTFs, `OFL.txt`,
  `METADATA.pb`, `DESCRIPTION.en_us.html` and/or
  `article/ARTICLE.en_us.html` (article supersedes description when it
  has images).
- [AUTO] `METADATA.pb` scaffolded by `gftools add-font`. Fields: name,
  designer, license "OFL", category (SERIF/SANS_SERIF/DISPLAY/
  HANDWRITING/MONOSPACE), date_added, per-font blocks, subsets
  (alphabetical, must include "menu"), primary_script, stroke,
  classifications, sample_text, minisite_url, and
  `source { repository_url, commit }` — GF won't accept families
  without a public repository_url + commit. (`upstream.yaml` is
  deprecated in favor of the METADATA.pb source block.)
- [HUMAN] category, designer names, description/article prose (~500
  words, third person, must link upstream repo, restricted HTML;
  img/video only in ARTICLE), designer profile, sample_text.
- [AUTO] QA gate: **zero FAILs on the googlefonts profile** —
  `fontspector -p googlefonts fonts/ttf/Family-*.ttf` (fontspector has
  replaced fontbakery in the GF flow; same profile name). Regression /
  proofing: `diffenator2 proof|diff`, or the wrapper
  `gftools qa -f *.ttf -a --rust` (`-gfb` compares against the served
  GF version). Running in CI is [AUTO]; dispositioning WARNs is [HUMAN].

## Summary: automation boundary for img2ufo

**Fully automatable:** fsType, name IDs 13/14, version format, the whole
vertical-metrics strategy, RIBBI naming mechanics, weight classes,
integer coords/extrema/closed contours/direction/overlap policy, Latin
Core coverage *verification*, repo scaffold + config.yaml, METADATA.pb
scaffold, fontspector-clean output.

**Irreducibly human:** family name choice, copyright holder/year/repo
URL, license status of the source specimen image, designing missing
glyphs, category/designer/description prose, the google/fonts issue +
curatorial acceptance, and WARN triage.

---

# fontspector integration

Researched 2026-07-05. Repo: https://github.com/fonttools/fontspector

## What it is
Rust port of fontbakery ("Skrifa/Read-Fonts-based font QA tool,
successor to fontbakery"), lead dev Simon Cozens, partially funded by
Google, built on the fontations stack. Latest release v1.7.2
(2026-06-26). It is effectively the **official GF QA tool now**:
fontbakery is frozen ("will continue to exist, but no longer updated" —
https://fontwerk.com/en/text/fontspector), the gf-guide onboarder
workflow references "the Fontspector report", and google/fonts CI uses
`fonttools/setup-fontspector`. ~1000x faster than fontbakery.

## Install
(https://github.com/fonttools/fontspector/blob/main/INSTALLATION.md)
- Prebuilt binaries on GitHub Releases (macOS aarch64/x86_64, Linux,
  Windows), or `cargo-binstall fontspector`, or
  `cargo install fontspector` (optional `--features python` for the
  fontbakery bridge). No pip/Homebrew path.
- Browser/WASM version at https://fonttools.github.io/fontspector/
  ("99% of functionality", fully client-side).

## CLI
- `fontspector -p googlefonts fonts/ttf/Family-*.ttf`
  (`--profile/-p`, default `universal`).
- Reporters (combinable file flags): `--json FILE`, `--csv FILE`,
  `--ghmarkdown FILE`, `--html FILE`, `--badges DIR`.
- Check selection: `--checkid/-c <substr>`, `--exclude_checkid/-x`;
  config via `--configuration file.toml`.
- Exit code: nonzero on any FAIL by default; threshold adjustable with
  `--error_code_on <severity>` — this is the CI gate mechanism.
- `--use-python` runs unported fontbakery checks via
  `fontspector-fontbakery-bridge`.

## Profiles
Built in: `universal` (default), `googlefonts`, `opentype`.
Runtime-loadable: `microsoft`, `adobe`, TOML-defined profiles
(typenetwork, fontwerk). The googlefonts profile covers fontbakery's
territory (METADATA.pb, name tables, OS/2, metrics, STAT, hinting,
glyph coverage); GF switched CI over and froze the fontbakery profile,
so parity is effectively official. Known gap: fontbakery's UFO-source
checks (ufolint, ufo_required_fields, etc.) are still unported
(commented out in profile-universal); the python bridge is the escape
hatch.

## Wiring into img2ufo (Rust pipeline)
- **Realistic today: shell out to the CLI** on the *compiled binary*:
  1. img2ufo emits UFO + designspace + config.yaml
  2. compile: fontmake (or fontc) → TTF
  3. gate: `fontspector -p googlefonts --json report.json
     --error_code_on fail Family[wght].ttf` → parse JSON, fail the
     pipeline on FAILs, surface WARNs for human triage.
- Library embedding is real but couples to fast-moving internals:
  crates `fontspector-checkapi` (Testable/Registry/Profile/Context),
  `fontspector-checkhelper` (`#[check(...)]` proc macro), plus
  `fontspector-profile-universal` / `-googlefonts` / `-opentype` on
  crates.io. The WASM build proves CLI-free embedding works, but there
  is no stable `run_qa(path) -> report` facade yet. Revisit for a
  future in-process gate.
- **Sources vs binaries:** fontspector is overwhelmingly a binary
  checker (read-fonts). A `profile-designspace` crate exists (norad-
  based checks: designspace_has_default_master, consistent glyphset/
  codepoints, path_direction) but `.ufo` CLI input is currently
  disabled. So: always compile first, check the TTF.
- **Custom checks:** write a Rust fn
  `(t: &Testable, context: &Context) -> CheckFnResult` with
  `#[check(id, rationale, proposal, title)]`, group into a Plugin;
  compile in or ship as a runtime-loadable `.fontspectorplugin` dylib
  (template: `profile-testplugin` in the repo). This is the natural
  home for img2ufo-specific checks (e.g. img2bez's structural gates
  repackaged as a fontspector plugin — also a visibility play).
- **CI:** `uses: fonttools/setup-fontspector@main` with `version:` a
  release tag; run with `--ghmarkdown` for PR summaries; google/fonts'
  own QA workflow is the reference implementation.


# img2ufo pipeline architecture

2026-07-05. Specimen image -> Google Fonts-onboardable UFO. All Rust.
First target corpus: ~/Desktop/letraset/ sheets.

## Stages

1. **Segment** — img2glyph (exists): adaptive threshold, connected
   components, reading-order manifest, unicode labeling. Output:
   labeled glyph PNGs (~100-200 px tall from letraset sheets).
2. **Resolution bridge** — the open question, two candidates raced
   against each other (A/B by final judge + fontspector results):
   a. LOCAL LoRA/SR: domain-specific glyph upscaler trained on our
      corpus render pairs (1000px + downscaled inputs; the degrade
      run's d5 variants are the training inputs). Gate: cycle-
      consistency (downsample output, diff vs original) + judge
      always scores the trace against ORIGINAL pixels.
   b. NO BRIDGE: trace native-res with the degradation-trained site
      head. Cheaper, deterministic; the stress test says diagonals
      wobble at ~170px, so (a) likely wins on quality.
3. **Trace** — img2bez trace_glyph per PNG (unicode from the img2glyph
   manifest drives the RTL start rule + glyph naming).
4. **Assemble** — img2ufo core: norad UFO build; fontinfo (family
   name, OFL fields per gf-guide), vertical metrics strategy from
   the gf-guide checklist (docs/gf-compliance-checklist.md), glyph
   set completion report (GF Latin Core coverage vs what the
   specimen provided), spacing v0 (sidebearings from ink bounds +
   corpus-calibrated defaults; learned spacing head later), kerning
   deferred to the learned head.
5. **QA gate** — fontspector (googlefonts profile) on the compiled
   font (fontc or fontmake for compile during development; fontc
   preferred, Rust). The pipeline FAILS on fontspector FAILs the
   same way img2bez fails its structural gate: no silent shipping.
6. **Specimens** — designbot renders proof sheets + the marketing
   specimen from the built font.

## Principles (inherited from img2bez)

- Deterministic pipeline decides geometry; generative models only
  assist perception (stage 2) and are verified against originals.
- Every stage emits a machine-readable report; the pipeline is an
  agent-runnable loop like the eval harness.
- GF compliance is a CHECKED property (fontspector + the checklist),
  not an aspiration.

## Near-term build order

- [ ] img2glyph pass over letraset sheets; eyeball segmentation
      quality + manifest labels.
- [ ] Wire stage 3+4 minimal: manifest -> traced UFO with default
      metrics (no bridge). End-to-end draft font THIS WEEK (launch
      demo material).
- [ ] fontspector gate wired in (stage 5).
- [ ] Bridge A/B once the SR model exists (frontier plan item).
- [ ] Spacing v0 from corpus sidebearing statistics.

## Output repo template: virtua-grotesk

Reference (per Eli, 2026-07-05): ~/GH/repos/virtua-grotesk — his
GF-workflow model repo. img2ufo's stage 4 emits an upstream repo
shaped like it, and its GOOGLE_FONTS_PORTING_CHECKLIST.md is the
copy-the-system contract to follow:

- sources/: <Family>-<Style>.ufo + <Family>.designspace +
  config.yaml (gftools builder) + sources/README.md
- fonts/ gitignored (built artifacts: ttf/, variable/)
- OFL.txt, AUTHORS.txt, CONTRIBUTORS.txt, README.md, Makefile +
  build.sh, documentation/
- AGENTS.md/CLAUDE.md for the agent workflow (img2bez pattern)

So the full contract: img2ufo(letraset sheet) -> a repo passing both
fontspector's googlefonts profile AND every unchecked box in the
porting checklist.

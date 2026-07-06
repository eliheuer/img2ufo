//! Auto-composition stage: build the missing GF Latin Core accented glyphs
//! as UFO composites (base component + mark component positioned by
//! anchors), and emit a machine-readable completion worklist for whatever
//! is left.
//!
//! Honesty policy (docs/gf-compliance-checklist.md section 3): a composite
//! is only built when the font actually contains its base and its mark.
//! The only ink this stage ever creates is ink *removal* (dotlessi /
//! dotlessj = i / j minus the tittle) and re-referencing existing ink
//! (combining mark <-> spacing accent aliases as zero-cost components).
//! Everything else that is missing is reported in the worklist, ranked so
//! a downstream agent (or human) knows which mark unlocks how many
//! composites — never faked.
//!
//! Anchor conventions (Glyphs/UFO ecosystem standard):
//! - base glyphs carry `top`, `bottom`, and where applicable `ogonek` /
//!   `topright`; marks carry the matching `_top` / `_bottom` / `_ogonek` /
//!   `_topright`. Composites place the mark at (base anchor - mark anchor).
//! - hand-placed anchors win: anchors are only added where a glyph does
//!   not already have one of that name, so green (hand-corrected) glyphs
//!   keep their corrections.

use crate::gf_latin_core;
use crate::pipeline::PipelineConfig;
use anyhow::{anyhow, Result};
use img2bez::kurbo::{Rect, Shape};
use img2bez::norad::{AffineTransform, Anchor, Component, Font, Glyph, Name};
use plist::Value;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Mark color for composite glyphs (yellow — generated, cheap to rebuild).
const COMPOSITE_MARK: &str = "1,1,0,1";
/// Mark color for derived-ink glyphs (orange — dotless forms, mark aliases).
const DERIVED_MARK: &str = "1,0.5,0,1";

// ============================================================================
// Recipes
// ============================================================================

/// Anchor class connecting a base anchor to a mark anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorClass {
    /// Mark centered above the base (`top` / `_top`).
    Top,
    /// Mark below the base (`bottom` / `_bottom`): cedilla, comma accent.
    Bottom,
    /// Ogonek hook at the base's bottom-right terminal (`ogonek` / `_ogonek`).
    Ogonek,
    /// Comma-shaped alt caron to the right of tall stems (`topright` /
    /// `_topright`): dcaron, lcaron, Lcaron, tcaron.
    TopRight,
}

impl AnchorClass {
    fn base_anchor(self) -> &'static str {
        match self {
            AnchorClass::Top => "top",
            AnchorClass::Bottom => "bottom",
            AnchorClass::Ogonek => "ogonek",
            AnchorClass::TopRight => "topright",
        }
    }
    fn mark_anchor(self) -> &'static str {
        match self {
            AnchorClass::Top => "_top",
            AnchorClass::Bottom => "_bottom",
            AnchorClass::Ogonek => "_ogonek",
            AnchorClass::TopRight => "_topright",
        }
    }
}

/// One composite: `name` = `base` + `mark` attached via `class`.
pub struct Recipe {
    pub name: &'static str,
    pub base: &'static str,
    pub mark: &'static str,
    pub class: AnchorClass,
}

macro_rules! recipes {
    ($($name:literal = $base:literal + $mark:literal @ $class:ident;)*) => {
        &[$(Recipe {
            name: $name,
            base: $base,
            mark: $mark,
            class: AnchorClass::$class,
        }),*]
    };
}

/// Every GF Latin Core precomposed glyph that is a base + single mark.
/// Deliberately absent (design work, not composition): AE OE Eth Thorn
/// Dcroat Hbar Lslash Oslash Germandbls germandbls eth ae oe dcroat hbar
/// lslash oslash thorn ordfeminine ordmasculine, and all atomic
/// punctuation/symbols.
pub const RECIPES: &[Recipe] = recipes![
    // --- Uppercase ---
    "Agrave" = "A" + "gravecomb" @ Top;
    "Aacute" = "A" + "acutecomb" @ Top;
    "Acircumflex" = "A" + "circumflexcomb" @ Top;
    "Atilde" = "A" + "tildecomb" @ Top;
    "Adieresis" = "A" + "dieresiscomb" @ Top;
    "Aring" = "A" + "ringcomb" @ Top;
    "Amacron" = "A" + "macroncomb" @ Top;
    "Abreve" = "A" + "brevecomb" @ Top;
    "Aogonek" = "A" + "ogonekcomb" @ Ogonek;
    "Cacute" = "C" + "acutecomb" @ Top;
    "Cdotaccent" = "C" + "dotaccentcomb" @ Top;
    "Ccaron" = "C" + "caroncomb" @ Top;
    "Ccedilla" = "C" + "cedillacomb" @ Bottom;
    "Dcaron" = "D" + "caroncomb" @ Top;
    "Egrave" = "E" + "gravecomb" @ Top;
    "Eacute" = "E" + "acutecomb" @ Top;
    "Ecircumflex" = "E" + "circumflexcomb" @ Top;
    "Edieresis" = "E" + "dieresiscomb" @ Top;
    "Emacron" = "E" + "macroncomb" @ Top;
    "Edotaccent" = "E" + "dotaccentcomb" @ Top;
    "Ecaron" = "E" + "caroncomb" @ Top;
    "Eogonek" = "E" + "ogonekcomb" @ Ogonek;
    "Gbreve" = "G" + "brevecomb" @ Top;
    "Gdotaccent" = "G" + "dotaccentcomb" @ Top;
    "Gcommaaccent" = "G" + "commaaccentcomb" @ Bottom;
    "Igrave" = "I" + "gravecomb" @ Top;
    "Iacute" = "I" + "acutecomb" @ Top;
    "Icircumflex" = "I" + "circumflexcomb" @ Top;
    "Idieresis" = "I" + "dieresiscomb" @ Top;
    "Imacron" = "I" + "macroncomb" @ Top;
    "Idotaccent" = "I" + "dotaccentcomb" @ Top;
    "Iogonek" = "I" + "ogonekcomb" @ Ogonek;
    "Kcommaaccent" = "K" + "commaaccentcomb" @ Bottom;
    "Lacute" = "L" + "acutecomb" @ Top;
    "Lcommaaccent" = "L" + "commaaccentcomb" @ Bottom;
    "Lcaron" = "L" + "caroncomb.alt" @ TopRight;
    "Nacute" = "N" + "acutecomb" @ Top;
    "Ntilde" = "N" + "tildecomb" @ Top;
    "Ncaron" = "N" + "caroncomb" @ Top;
    "Ncommaaccent" = "N" + "commaaccentcomb" @ Bottom;
    "Ograve" = "O" + "gravecomb" @ Top;
    "Oacute" = "O" + "acutecomb" @ Top;
    "Ocircumflex" = "O" + "circumflexcomb" @ Top;
    "Otilde" = "O" + "tildecomb" @ Top;
    "Odieresis" = "O" + "dieresiscomb" @ Top;
    "Ohungarumlaut" = "O" + "hungarumlautcomb" @ Top;
    "Racute" = "R" + "acutecomb" @ Top;
    "Rcaron" = "R" + "caroncomb" @ Top;
    "Sacute" = "S" + "acutecomb" @ Top;
    "Scaron" = "S" + "caroncomb" @ Top;
    "Scedilla" = "S" + "cedillacomb" @ Bottom;
    "Scommaaccent" = "S" + "commaaccentcomb" @ Bottom;
    "Tcaron" = "T" + "caroncomb" @ Top;
    "Tcommaaccent" = "T" + "commaaccentcomb" @ Bottom;
    "Ugrave" = "U" + "gravecomb" @ Top;
    "Uacute" = "U" + "acutecomb" @ Top;
    "Ucircumflex" = "U" + "circumflexcomb" @ Top;
    "Udieresis" = "U" + "dieresiscomb" @ Top;
    "Umacron" = "U" + "macroncomb" @ Top;
    "Uring" = "U" + "ringcomb" @ Top;
    "Uhungarumlaut" = "U" + "hungarumlautcomb" @ Top;
    "Uogonek" = "U" + "ogonekcomb" @ Ogonek;
    "Wgrave" = "W" + "gravecomb" @ Top;
    "Wacute" = "W" + "acutecomb" @ Top;
    "Wcircumflex" = "W" + "circumflexcomb" @ Top;
    "Wdieresis" = "W" + "dieresiscomb" @ Top;
    "Ygrave" = "Y" + "gravecomb" @ Top;
    "Yacute" = "Y" + "acutecomb" @ Top;
    "Ycircumflex" = "Y" + "circumflexcomb" @ Top;
    "Ydieresis" = "Y" + "dieresiscomb" @ Top;
    "Zacute" = "Z" + "acutecomb" @ Top;
    "Zdotaccent" = "Z" + "dotaccentcomb" @ Top;
    "Zcaron" = "Z" + "caroncomb" @ Top;
    // --- Lowercase ---
    "agrave" = "a" + "gravecomb" @ Top;
    "aacute" = "a" + "acutecomb" @ Top;
    "acircumflex" = "a" + "circumflexcomb" @ Top;
    "atilde" = "a" + "tildecomb" @ Top;
    "adieresis" = "a" + "dieresiscomb" @ Top;
    "aring" = "a" + "ringcomb" @ Top;
    "amacron" = "a" + "macroncomb" @ Top;
    "abreve" = "a" + "brevecomb" @ Top;
    "aogonek" = "a" + "ogonekcomb" @ Ogonek;
    "cacute" = "c" + "acutecomb" @ Top;
    "cdotaccent" = "c" + "dotaccentcomb" @ Top;
    "ccaron" = "c" + "caroncomb" @ Top;
    "ccedilla" = "c" + "cedillacomb" @ Bottom;
    "dcaron" = "d" + "caroncomb.alt" @ TopRight;
    "egrave" = "e" + "gravecomb" @ Top;
    "eacute" = "e" + "acutecomb" @ Top;
    "ecircumflex" = "e" + "circumflexcomb" @ Top;
    "edieresis" = "e" + "dieresiscomb" @ Top;
    "emacron" = "e" + "macroncomb" @ Top;
    "edotaccent" = "e" + "dotaccentcomb" @ Top;
    "ecaron" = "e" + "caroncomb" @ Top;
    "eogonek" = "e" + "ogonekcomb" @ Ogonek;
    "gbreve" = "g" + "brevecomb" @ Top;
    "gdotaccent" = "g" + "dotaccentcomb" @ Top;
    "gcommaaccent" = "g" + "commaturnedabovecomb" @ Top;
    "igrave" = "dotlessi" + "gravecomb" @ Top;
    "iacute" = "dotlessi" + "acutecomb" @ Top;
    "icircumflex" = "dotlessi" + "circumflexcomb" @ Top;
    "idieresis" = "dotlessi" + "dieresiscomb" @ Top;
    "imacron" = "dotlessi" + "macroncomb" @ Top;
    "iogonek" = "i" + "ogonekcomb" @ Ogonek;
    "kcommaaccent" = "k" + "commaaccentcomb" @ Bottom;
    "lacute" = "l" + "acutecomb" @ Top;
    "lcommaaccent" = "l" + "commaaccentcomb" @ Bottom;
    "lcaron" = "l" + "caroncomb.alt" @ TopRight;
    "nacute" = "n" + "acutecomb" @ Top;
    "ntilde" = "n" + "tildecomb" @ Top;
    "ncaron" = "n" + "caroncomb" @ Top;
    "ncommaaccent" = "n" + "commaaccentcomb" @ Bottom;
    "ograve" = "o" + "gravecomb" @ Top;
    "oacute" = "o" + "acutecomb" @ Top;
    "ocircumflex" = "o" + "circumflexcomb" @ Top;
    "otilde" = "o" + "tildecomb" @ Top;
    "odieresis" = "o" + "dieresiscomb" @ Top;
    "ohungarumlaut" = "o" + "hungarumlautcomb" @ Top;
    "racute" = "r" + "acutecomb" @ Top;
    "rcaron" = "r" + "caroncomb" @ Top;
    "sacute" = "s" + "acutecomb" @ Top;
    "scaron" = "s" + "caroncomb" @ Top;
    "scedilla" = "s" + "cedillacomb" @ Bottom;
    "scommaaccent" = "s" + "commaaccentcomb" @ Bottom;
    "tcaron" = "t" + "caroncomb.alt" @ TopRight;
    "tcommaaccent" = "t" + "commaaccentcomb" @ Bottom;
    "ugrave" = "u" + "gravecomb" @ Top;
    "uacute" = "u" + "acutecomb" @ Top;
    "ucircumflex" = "u" + "circumflexcomb" @ Top;
    "udieresis" = "u" + "dieresiscomb" @ Top;
    "umacron" = "u" + "macroncomb" @ Top;
    "uring" = "u" + "ringcomb" @ Top;
    "uhungarumlaut" = "u" + "hungarumlautcomb" @ Top;
    "uogonek" = "u" + "ogonekcomb" @ Ogonek;
    "wgrave" = "w" + "gravecomb" @ Top;
    "wacute" = "w" + "acutecomb" @ Top;
    "wcircumflex" = "w" + "circumflexcomb" @ Top;
    "wdieresis" = "w" + "dieresiscomb" @ Top;
    "ygrave" = "y" + "gravecomb" @ Top;
    "yacute" = "y" + "acutecomb" @ Top;
    "ycircumflex" = "y" + "circumflexcomb" @ Top;
    "ydieresis" = "y" + "dieresiscomb" @ Top;
    "zacute" = "z" + "acutecomb" @ Top;
    "zdotaccent" = "z" + "dotaccentcomb" @ Top;
    "zcaron" = "z" + "caroncomb" @ Top;
];

/// (spacing accent, combining mark) pairs: when the font has one, the other
/// is derived as a component alias (same ink, no new design).
pub const MARK_PAIRS: &[(&str, &str)] = &[
    ("grave", "gravecomb"),
    ("acute", "acutecomb"),
    ("dieresis", "dieresiscomb"),
    ("macron", "macroncomb"),
    ("circumflex", "circumflexcomb"),
    ("caron", "caroncomb"),
    ("breve", "brevecomb"),
    ("dotaccent", "dotaccentcomb"),
    ("ring", "ringcomb"),
    ("ogonek", "ogonekcomb"),
    ("tilde", "tildecomb"),
    ("hungarumlaut", "hungarumlautcomb"),
    ("cedilla", "cedillacomb"),
];

// ============================================================================
// Completion report (the agent worklist)
// ============================================================================

#[derive(Serialize, Clone, Default)]
pub struct Coverage {
    pub covered: usize,
    pub total: usize,
}

#[derive(Serialize, Clone)]
pub struct BuiltComposite {
    pub name: String,
    pub base: String,
    pub mark: String,
    pub anchor: String,
}

#[derive(Serialize, Clone)]
pub struct DerivedGlyph {
    pub name: String,
    pub from: String,
    pub method: String,
}

#[derive(Serialize, Clone)]
pub struct MissingMark {
    /// Glyph name to draw (e.g. "acutecomb", "caroncomb.alt").
    pub name: String,
    /// "U+XXXX" if the mark itself is a Latin Core cmap entry.
    pub codepoint: Option<String>,
    /// Composites (and mark aliases) that become buildable once this
    /// mark exists. Sort key for the worklist.
    pub unlocks: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct MissingAtomic {
    pub name: String,
    pub codepoint: String,
}

/// Machine-readable completion worklist, written next to the UFO. This is
/// the contract for a downstream glyph-completion agent: draw the
/// `missing_marks` (highest unlock count first) and `missing_atomic`
/// glyphs, feed them back through the pipeline, and composition finishes
/// the rest.
#[derive(Serialize, Clone, Default)]
pub struct CompletionReport {
    pub family: String,
    pub glyphset: String,
    pub coverage_traced: Coverage,
    pub coverage_after_composition: Coverage,
    pub built_composites: Vec<BuiltComposite>,
    pub derived_glyphs: Vec<DerivedGlyph>,
    pub missing_marks: Vec<MissingMark>,
    pub missing_atomic: Vec<MissingAtomic>,
}

/// Everything the composition stage produced.
pub struct Composition {
    pub report: CompletionReport,
    /// Ink extremes contributed by composites (for win metrics).
    pub ink_y_min: f64,
    pub ink_y_max: f64,
}

// ============================================================================
// Geometry helpers
// ============================================================================

/// Exact ink bounds of a glyph (curves, not control boxes), resolving
/// components up to 3 levels deep.
fn ink_bounds(font: &Font, glyph: &Glyph, depth: u8) -> Option<Rect> {
    let mut bounds: Option<Rect> = None;
    let mut merge = |r: Rect| {
        bounds = Some(match bounds {
            Some(b) => b.union(r),
            None => r,
        });
    };
    for contour in &glyph.contours {
        if let Ok(path) = contour.to_kurbo() {
            if !path.elements().is_empty() {
                merge(path.bounding_box());
            }
        }
    }
    if depth < 3 {
        for comp in &glyph.components {
            if let Some(base) = font.default_layer().get_glyph(&comp.base) {
                if let Some(b) = ink_bounds(font, base, depth + 1) {
                    let t = &comp.transform;
                    // Transform all four corners (handles offsets + scales).
                    let corners = [
                        (b.x0, b.y0),
                        (b.x1, b.y0),
                        (b.x0, b.y1),
                        (b.x1, b.y1),
                    ];
                    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
                    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
                    for (x, y) in corners {
                        let tx = t.x_scale * x + t.yx_scale * y + t.x_offset;
                        let ty = t.xy_scale * x + t.y_scale * y + t.y_offset;
                        x0 = x0.min(tx);
                        y0 = y0.min(ty);
                        x1 = x1.max(tx);
                        y1 = y1.max(ty);
                    }
                    merge(Rect::new(x0, y0, x1, y1));
                }
            }
        }
    }
    bounds
}

fn snap(v: f64, grid: i32) -> f64 {
    if grid > 1 {
        (v / grid as f64).round() * grid as f64
    } else {
        v.round()
    }
}

fn offset_transform(dx: f64, dy: f64) -> AffineTransform {
    AffineTransform {
        x_scale: 1.0,
        xy_scale: 0.0,
        yx_scale: 0.0,
        y_scale: 1.0,
        x_offset: dx,
        y_offset: dy,
    }
}

fn static_name(s: &str) -> Name {
    Name::new(s).expect("static glyph/anchor names are valid")
}

fn has_anchor(glyph: &Glyph, name: &str) -> bool {
    glyph
        .anchors
        .iter()
        .any(|a| a.name.as_ref().map(|n| n.as_str()) == Some(name))
}

fn anchor_pos(glyph: &Glyph, name: &str) -> Option<(f64, f64)> {
    glyph
        .anchors
        .iter()
        .find(|a| a.name.as_ref().map(|n| n.as_str()) == Some(name))
        .map(|a| (a.x, a.y))
}

fn codepoint_for(name: &str) -> Option<u32> {
    gf_latin_core::GLYPHSET
        .iter()
        .find(|&&(_, n)| n == name)
        .map(|&(cp, _)| cp)
}

fn set_mark_color(glyph: &mut Glyph, color: &str) {
    glyph
        .lib
        .insert("public.markColor".into(), Value::String(color.into()));
}

fn is_green(existing: Option<&Font>, name: &str) -> Option<Glyph> {
    let font = existing?;
    let glyph = font.default_layer().get_glyph(name)?;
    let mark = glyph
        .lib
        .get("public.markColor")
        .and_then(|v| v.as_string())
        .unwrap_or("");
    mark.starts_with(crate::ufo_builder::GREEN_MARK_PREFIX)
        .then(|| glyph.clone())
}

// ============================================================================
// The stage
// ============================================================================

/// Run auto-composition on a freshly built font. Mutates `font` (derived
/// glyphs, anchors, composites), `encoded` (new cmap entries), and
/// `glyph_order`. `existing` is the previous UFO build, for green-glyph
/// preservation.
pub fn run(
    font: &mut Font,
    existing: Option<&Font>,
    config: &PipelineConfig,
    encoded: &mut HashSet<u32>,
    glyph_order: &mut Vec<String>,
) -> Result<Composition> {
    let coverage_traced = coverage_of(encoded);
    let grid = config.grid;
    let upm = config.upm as f64;
    // Vertical clearance between base ink and a floating mark's ink.
    let gap = snap(upm * 0.07, grid);
    // Horizontal clearance for topright (alt caron) attachment.
    let h_gap = snap(upm * 0.03, grid);

    let mut derived: Vec<DerivedGlyph> = Vec::new();
    let mut built: Vec<BuiltComposite> = Vec::new();
    let mut ink_y_min = f64::MAX;
    let mut ink_y_max = f64::MIN;

    let has = |font: &Font, name: &str| font.default_layer().get_glyph(name).is_some();

    // ------------------------------------------------------------------
    // 1. Derive dotless forms (ink removal: drop the tittle).
    // ------------------------------------------------------------------
    for (dotless, source, cp) in [("dotlessi", "i", 0x0131_u32), ("dotlessj", "j", 0x0237)] {
        if has(font, dotless) {
            continue;
        }
        if let Some(kept) = is_green(existing, dotless) {
            font.default_layer_mut().insert_glyph(kept);
            record_glyph(dotless, cp, encoded, glyph_order);
            continue;
        }
        let Some(source_glyph) = font.default_layer().get_glyph(source) else {
            continue;
        };
        match derive_dotless(source_glyph, dotless, cp, config) {
            Some(glyph) => {
                font.default_layer_mut().insert_glyph(glyph);
                record_glyph(dotless, cp, encoded, glyph_order);
                derived.push(DerivedGlyph {
                    name: dotless.into(),
                    from: source.into(),
                    method: "tittle removed".into(),
                });
            }
            None => {
                // Tittle not separable; the worklist will list it as atomic.
            }
        }
    }

    // ------------------------------------------------------------------
    // 2. Mark aliases: combining <-> spacing, whichever direction the
    //    specimen provided.
    // ------------------------------------------------------------------
    for &(spacing, comb) in MARK_PAIRS {
        let have_spacing = has(font, spacing);
        let have_comb = has(font, comb);
        if have_spacing == have_comb {
            continue; // both present or both absent
        }
        let (new_name, from, cp) = if have_spacing {
            (comb, spacing, codepoint_for(comb))
        } else {
            (spacing, comb, codepoint_for(spacing))
        };
        if let Some(kept) = is_green(existing, new_name) {
            font.default_layer_mut().insert_glyph(kept);
            if let Some(cp) = cp {
                record_glyph(new_name, cp, encoded, glyph_order);
            } else {
                glyph_order.push(new_name.into());
            }
            continue;
        }
        let source_glyph = font
            .default_layer()
            .get_glyph(from)
            .expect("presence checked above");
        let Some(b) = ink_bounds(font, source_glyph, 0) else {
            continue;
        };
        let mut glyph = Glyph::new(new_name);
        if have_spacing {
            // Combining mark: zero width, ink centered on x=0.
            let dx = snap(-(b.x0 + b.x1) / 2.0, grid);
            glyph.width = 0.0;
            glyph
                .components
                .push(Component::new(static_name(from), offset_transform(dx, 0.0), None));
        } else {
            // Spacing accent: modest sidebearings around the comb ink.
            let lsb = snap(upm * 0.04, grid);
            let dx = snap(lsb - b.x0, grid);
            glyph.width = snap(b.width() + 2.0 * lsb, grid);
            glyph
                .components
                .push(Component::new(static_name(from), offset_transform(dx, 0.0), None));
        }
        if let Some(cp) = cp {
            glyph
                .codepoints
                .insert(char::from_u32(cp).ok_or_else(|| anyhow!("bad codepoint for {new_name}"))?);
        }
        set_mark_color(&mut glyph, DERIVED_MARK);
        font.default_layer_mut().insert_glyph(glyph);
        if let Some(cp) = cp {
            record_glyph(new_name, cp, encoded, glyph_order);
        } else {
            glyph_order.push(new_name.into());
        }
        derived.push(DerivedGlyph {
            name: new_name.into(),
            from: from.into(),
            method: "component alias".into(),
        });
    }

    // ------------------------------------------------------------------
    // 3. Anchors. Bases: every anchor class any recipe wants from them
    //    (top/bottom always for plain letters). Marks: their attachment
    //    anchor. Existing anchors of the same name are never touched.
    // ------------------------------------------------------------------
    let mut base_classes: HashMap<&str, Vec<AnchorClass>> = HashMap::new();
    let mut mark_classes: HashMap<&str, AnchorClass> = HashMap::new();
    for r in RECIPES {
        let classes = base_classes.entry(r.base).or_default();
        if !classes.contains(&r.class) {
            classes.push(r.class);
        }
        mark_classes.entry(r.mark).or_insert(r.class);
    }

    for (&base, classes) in &base_classes {
        let Some(glyph) = font.default_layer().get_glyph(base) else {
            continue;
        };
        let Some(b) = ink_bounds(font, glyph, 0) else {
            continue;
        };
        let mut new_anchors: Vec<Anchor> = Vec::new();
        for class in classes {
            let name = class.base_anchor();
            if has_anchor(glyph, name) {
                continue;
            }
            let (x, y) = match class {
                // On the letter's ink: mark clearance lives in the mark's
                // `_top` anchor.
                AnchorClass::Top => ((b.x0 + b.x1) / 2.0, b.y1),
                AnchorClass::Bottom => ((b.x0 + b.x1) / 2.0, b.y0),
                // Bottom-right terminal, on the baseline.
                AnchorClass::Ogonek => (b.x1 - snap(upm * 0.015, grid), 0.0),
                AnchorClass::TopRight => (b.x1, b.y1),
            };
            new_anchors.push(Anchor::new(
                snap(x, grid),
                snap(y, grid),
                Some(static_name(name)),
                None,
                None,
            ));
        }
        if !new_anchors.is_empty() {
            let glyph = font
                .default_layer_mut()
                .get_glyph_mut(base)
                .expect("presence checked above");
            glyph.anchors.extend(new_anchors);
        }
    }

    for (&mark, &class) in &mark_classes {
        let Some(glyph) = font.default_layer().get_glyph(mark) else {
            continue;
        };
        let Some(b) = ink_bounds(font, glyph, 0) else {
            continue;
        };
        let name = class.mark_anchor();
        if has_anchor(glyph, name) {
            continue;
        }
        let cx = (b.x0 + b.x1) / 2.0;
        let (x, y) = match (class, mark) {
            // Floating marks above: clearance baked in below the ink.
            (AnchorClass::Top, _) => (cx, b.y0 - gap),
            // Cedilla and ogonek connect to the base; comma accent floats.
            (AnchorClass::Bottom, "cedillacomb") => (cx, b.y1),
            (AnchorClass::Bottom, _) => (cx, b.y1 + gap),
            (AnchorClass::Ogonek, _) => (cx, b.y1),
            // Alt caron: left edge, top-aligned with the base.
            (AnchorClass::TopRight, _) => (b.x0 - h_gap, b.y1),
        };
        let glyph = font
            .default_layer_mut()
            .get_glyph_mut(mark)
            .expect("presence checked above");
        glyph.anchors.push(Anchor::new(
            snap(x, grid),
            snap(y, grid),
            Some(static_name(name)),
            None,
            None,
        ));
    }

    // ------------------------------------------------------------------
    // 4. Compose.
    // ------------------------------------------------------------------
    let mut blocked: HashMap<&str, Vec<&str>> = HashMap::new(); // recipe -> missing parts
    for r in RECIPES {
        let cp = codepoint_for(r.name)
            .ok_or_else(|| anyhow!("recipe target {} not in GF Latin Core table", r.name))?;
        if encoded.contains(&cp) || has(font, r.name) {
            continue;
        }
        if let Some(kept) = is_green(existing, r.name) {
            if let Some(b) = ink_bounds(font, &kept, 0) {
                ink_y_min = ink_y_min.min(b.y0);
                ink_y_max = ink_y_max.max(b.y1);
            }
            font.default_layer_mut().insert_glyph(kept);
            record_glyph(r.name, cp, encoded, glyph_order);
            continue;
        }

        let mut missing: Vec<&str> = Vec::new();
        if !has(font, r.base) {
            missing.push(r.base);
        }
        if !has(font, r.mark) {
            missing.push(r.mark);
        }
        if !missing.is_empty() {
            blocked.insert(r.name, missing);
            continue;
        }

        let base_glyph = font.default_layer().get_glyph(r.base).unwrap();
        let mark_glyph = font.default_layer().get_glyph(r.mark).unwrap();
        let (Some((bx, by)), Some((mx, my))) = (
            anchor_pos(base_glyph, r.class.base_anchor()),
            anchor_pos(mark_glyph, r.class.mark_anchor()),
        ) else {
            // Anchors could not be computed (empty ink); treat as blocked.
            blocked.insert(r.name, vec![r.mark]);
            continue;
        };
        let dx = snap(bx - mx, grid);
        let dy = snap(by - my, grid);

        let mut glyph = Glyph::new(r.name);
        glyph.width = base_glyph.width;
        glyph
            .codepoints
            .insert(char::from_u32(cp).ok_or_else(|| anyhow!("bad codepoint for {}", r.name))?);
        glyph.components.push(Component::new(
            static_name(r.base),
            offset_transform(0.0, 0.0),
            None,
        ));
        glyph.components.push(Component::new(
            static_name(r.mark),
            offset_transform(dx, dy),
            None,
        ));
        set_mark_color(&mut glyph, COMPOSITE_MARK);

        if let Some(b) = ink_bounds(font, base_glyph, 0) {
            ink_y_min = ink_y_min.min(b.y0);
            ink_y_max = ink_y_max.max(b.y1);
        }
        if let Some(b) = ink_bounds(font, mark_glyph, 0) {
            ink_y_min = ink_y_min.min(b.y0 + dy);
            ink_y_max = ink_y_max.max(b.y1 + dy);
        }

        font.default_layer_mut().insert_glyph(glyph);
        record_glyph(r.name, cp, encoded, glyph_order);
        built.push(BuiltComposite {
            name: r.name.into(),
            base: r.base.into(),
            mark: r.mark.into(),
            anchor: r.class.base_anchor().into(),
        });
    }

    // ------------------------------------------------------------------
    // 5. Categorize what is still missing (the worklist).
    // ------------------------------------------------------------------
    let mark_names: HashSet<&str> = MARK_PAIRS
        .iter()
        .flat_map(|&(s, c)| [s, c])
        .chain(mark_classes.keys().copied())
        .chain(["commaaccentcomb", "commaturnedabovecomb", "caroncomb.alt"])
        .collect();

    // Every glyph a missing mark would unlock, including its alias partner.
    let mut unlocks: HashMap<&str, Vec<String>> = HashMap::new();
    for (recipe_name, missing) in &blocked {
        for part in missing {
            unlocks
                .entry(part)
                .or_default()
                .push((*recipe_name).to_string());
        }
    }
    for &(spacing, comb) in MARK_PAIRS {
        if !has(font, spacing) && !has(font, comb) {
            unlocks.entry(comb).or_default().push(spacing.to_string());
        }
    }

    let mut missing_marks: Vec<MissingMark> = Vec::new();
    let mut missing_atomic: Vec<MissingAtomic> = Vec::new();
    for &(cp, name) in gf_latin_core::GLYPHSET {
        if encoded.contains(&cp) {
            continue;
        }
        if mark_names.contains(name) {
            // Marks whose alias partner is present were derived above, so
            // anything left here genuinely needs drawing. Fold spacing
            // accents into their combining form's entry (drawing either
            // derives the other).
            let comb = MARK_PAIRS
                .iter()
                .find(|&&(s, _)| s == name)
                .map(|&(_, c)| c);
            if comb.is_some() {
                continue; // reported under the combining name
            }
            let mut unlock_list = unlocks.remove(name).unwrap_or_default();
            unlock_list.sort();
            unlock_list.dedup();
            missing_marks.push(MissingMark {
                name: name.to_string(),
                codepoint: Some(format!("U+{cp:04X}")),
                unlocks: unlock_list,
            });
        } else {
            missing_atomic.push(MissingAtomic {
                name: name.to_string(),
                codepoint: format!("U+{cp:04X}"),
            });
        }
    }
    // Unencoded marks (caroncomb.alt, commaturnedabovecomb) and any other
    // blocked ingredient that is not a Latin Core cmap entry.
    for (part, unlock_list) in unlocks {
        if font.default_layer().get_glyph(part).is_some() {
            continue;
        }
        let mut unlock_list = unlock_list;
        unlock_list.sort();
        unlock_list.dedup();
        missing_marks.push(MissingMark {
            name: part.to_string(),
            codepoint: codepoint_for(part).map(|cp| format!("U+{cp:04X}")),
            unlocks: unlock_list,
        });
    }
    missing_marks.sort_by(|a, b| b.unlocks.len().cmp(&a.unlocks.len()).then(a.name.cmp(&b.name)));

    let report = CompletionReport {
        family: config.family.clone(),
        glyphset: "GF_Latin_Core".into(),
        coverage_traced,
        coverage_after_composition: coverage_of(encoded),
        built_composites: built,
        derived_glyphs: derived,
        missing_marks,
        missing_atomic,
    };

    Ok(Composition {
        report,
        ink_y_min,
        ink_y_max,
    })
}

fn coverage_of(encoded: &HashSet<u32>) -> Coverage {
    let (covered, total, _) = gf_latin_core::coverage(encoded);
    Coverage { covered, total }
}

fn record_glyph(name: &str, cp: u32, encoded: &mut HashSet<u32>, glyph_order: &mut Vec<String>) {
    encoded.insert(cp);
    glyph_order.push(name.to_string());
}

/// dotlessi/dotlessj: clone the source glyph without its tittle — the
/// contour whose ink sits entirely above all other contours. Returns None
/// when no such contour exists (single-story ink, connected tittle, ...).
fn derive_dotless(source: &Glyph, name: &str, cp: u32, config: &PipelineConfig) -> Option<Glyph> {
    if source.contours.len() < 2 {
        return None;
    }
    let boxes: Vec<Rect> = source
        .contours
        .iter()
        .map(|c| c.to_kurbo().ok().map(|p| p.bounding_box()))
        .collect::<Option<Vec<_>>>()?;

    // Candidate tittle: the contour with the highest bottom edge.
    let (tittle_idx, tittle_box) = boxes
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.y0.partial_cmp(&b.y0).unwrap())?;
    let body_top = boxes
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != tittle_idx)
        .map(|(_, b)| b.y1)
        .fold(f64::MIN, f64::max);

    // The tittle must clear the body and sit in the upper region.
    if tittle_box.y0 <= body_top || tittle_box.y0 < config.x_height as f64 * 0.75 {
        return None;
    }

    let mut glyph = Glyph::new(name);
    glyph.width = source.width;
    glyph.height = source.height;
    glyph.contours = source
        .contours
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != tittle_idx)
        .map(|(_, c)| c.clone())
        .collect();
    glyph.codepoints.insert(char::from_u32(cp)?);
    set_mark_color(&mut glyph, DERIVED_MARK);
    Some(glyph)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use img2bez::norad::{Contour, ContourPoint, PointType};

    fn test_config() -> PipelineConfig {
        PipelineConfig {
            input: "unused.png".into(),
            output: None,
            glyph_dir: None,
            labels: None,
            family: "Test".into(),
            style: "Regular".into(),
            designer: "Test".into(),
            git_url: None,
            upm: 1024,
            ascender: 832,
            descender: -256,
            x_height: 576,
            cap_height: 768,
            profile: img2bez::Profile::Clean,
            accuracy: None,
            grid: 2,
            min_area: 200,
            max_area: 50000,
            no_qa: true,
            emit_repo: None,
            verbose: false,
            copyright: String::new(),
        }
    }

    fn rect_contour(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
        let p = |x, y| ContourPoint::new(x, y, PointType::Line, false, None, None);
        Contour::new(vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)], None)
    }

    fn rect_glyph(name: &str, x0: f64, y0: f64, x1: f64, y1: f64, width: f64) -> Glyph {
        let mut g = Glyph::new(name);
        g.width = width;
        g.contours.push(rect_contour(x0, y0, x1, y1));
        g
    }

    #[test]
    fn recipe_targets_are_all_latin_core() {
        for r in RECIPES {
            assert!(
                codepoint_for(r.name).is_some(),
                "recipe target {} missing from GLYPHSET",
                r.name
            );
        }
    }

    #[test]
    fn recipes_and_pairs_have_no_duplicates() {
        let mut seen = HashSet::new();
        for r in RECIPES {
            assert!(seen.insert(r.name), "duplicate recipe for {}", r.name);
        }
        let mut seen = HashSet::new();
        for &(s, c) in MARK_PAIRS {
            assert!(seen.insert(s) && seen.insert(c), "duplicate mark pair {s}/{c}");
        }
    }

    #[test]
    fn composes_from_spacing_accent_via_alias() {
        let mut font = Font::new();
        font.default_layer_mut()
            .insert_glyph(rect_glyph("A", 0.0, 0.0, 600.0, 700.0, 640.0));
        // Spacing acute: ink 200..360 x 520..700, advance 400.
        font.default_layer_mut()
            .insert_glyph(rect_glyph("acute", 200.0, 520.0, 360.0, 700.0, 400.0));

        let mut encoded: HashSet<u32> = [0x0041].into();
        let mut order = vec!["A".to_string()];
        let comp = run(&mut font, None, &test_config(), &mut encoded, &mut order).unwrap();

        // acutecomb derived: zero width, ink centered on x=0.
        let comb = font.default_layer().get_glyph("acutecomb").expect("acutecomb derived");
        assert_eq!(comb.width, 0.0);
        assert_eq!(comb.components.len(), 1);
        assert_eq!(comb.components[0].transform.x_offset, -280.0);
        assert!(encoded.contains(&0x0301));

        // Aacute composed: base at identity, mark centered above.
        let aacute = font.default_layer().get_glyph("Aacute").expect("Aacute built");
        assert_eq!(aacute.width, 640.0);
        assert_eq!(aacute.components.len(), 2);
        let mark_comp = &aacute.components[1];
        // base top anchor: (300, 700); comb ink is -80..80 x 520..700 so
        // _top = (0, 520 - gap). gap = snap(1024*0.07) = 72 -> _top y 448.
        assert_eq!(mark_comp.transform.x_offset, 300.0);
        assert_eq!(mark_comp.transform.y_offset, 252.0);
        assert!(encoded.contains(&0x00C1));
        assert!(comp.report.built_composites.iter().any(|b| b.name == "Aacute"));
        // Composite ink top: comb top 700 + dy 252 = 952.
        assert_eq!(comp.ink_y_max, 952.0);

        // Blocked recipes report their missing mark, ranked by unlocks.
        assert!(comp
            .report
            .missing_marks
            .iter()
            .any(|m| m.name == "gravecomb" && m.unlocks.contains(&"Agrave".to_string())));
    }

    #[test]
    fn derives_dotless_forms() {
        let mut font = Font::new();
        let mut i = Glyph::new("i");
        i.width = 300.0;
        i.contours.push(rect_contour(100.0, 0.0, 200.0, 576.0)); // stem
        i.contours.push(rect_contour(100.0, 650.0, 200.0, 750.0)); // tittle
        font.default_layer_mut().insert_glyph(i);

        let mut encoded: HashSet<u32> = [0x0069].into();
        let mut order = vec!["i".to_string()];
        let comp = run(&mut font, None, &test_config(), &mut encoded, &mut order).unwrap();

        let dotless = font.default_layer().get_glyph("dotlessi").expect("dotlessi derived");
        assert_eq!(dotless.contours.len(), 1);
        assert_eq!(dotless.width, 300.0);
        assert!(encoded.contains(&0x0131));
        assert!(comp.report.derived_glyphs.iter().any(|d| d.name == "dotlessi"));
    }

    #[test]
    fn no_marks_means_no_composites_and_a_full_worklist() {
        let mut font = Font::new();
        font.default_layer_mut()
            .insert_glyph(rect_glyph("A", 0.0, 0.0, 600.0, 700.0, 640.0));
        let mut encoded: HashSet<u32> = [0x0041].into();
        let mut order = vec!["A".to_string()];
        let comp = run(&mut font, None, &test_config(), &mut encoded, &mut order).unwrap();

        assert!(comp.report.built_composites.is_empty());
        // acutecomb should top-rank near dieresiscomb/caroncomb by unlocks.
        let first = &comp.report.missing_marks[0];
        assert!(first.unlocks.len() >= 10, "expected high unlock count, got {}", first.unlocks.len());
    }
}

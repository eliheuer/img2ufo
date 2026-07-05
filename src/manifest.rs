//! Types for the manifest.json produced by img2glyph, plus the external
//! labels file (`--labels`) that maps glyph ids to Unicode codepoints.

use crate::gf_latin_core;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct BoundingBox {
    /// Kept to mirror the img2glyph schema; placement only needs y/h today.
    #[allow(dead_code)]
    pub x: u32,
    pub y: u32,
    #[allow(dead_code)]
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Deserialize)]
pub struct GlyphEntry {
    pub id: String,
    /// Path to the extracted glyph PNG, relative to the manifest file.
    pub file: PathBuf,
    pub bbox: BoundingBox,
    #[allow(dead_code)]
    pub area_px: u64,
    pub row: usize,
    #[allow(dead_code)]
    pub col: usize,
    /// Codepoint as "U+0041", if labeled (by img2glyph or --labels).
    pub unicode: Option<String>,
    /// Glyph name, if labeled. Derived from `unicode` when absent.
    pub glyph_name: Option<String>,
}

impl GlyphEntry {
    /// Parsed codepoint, if labeled.
    pub fn codepoint(&self) -> Option<char> {
        let hex = self.unicode.as_deref()?;
        let cp = u32::from_str_radix(hex.trim_start_matches("U+"), 16).ok()?;
        char::from_u32(cp)
    }
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub glyphs: Vec<GlyphEntry>,
}

impl Manifest {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let manifest: Manifest = serde_json::from_str(&data)?;
        Ok(manifest)
    }

    /// Merge an external labels file (glyph id -> unicode) into the manifest,
    /// then derive glyph names for every labeled entry that lacks one.
    pub fn apply_labels(&mut self, labels: &Labels) {
        for entry in &mut self.glyphs {
            if let Some(label) = labels.0.get(&entry.id) {
                if label.unicode.is_some() {
                    entry.unicode = label.unicode.clone();
                }
                if label.glyph_name.is_some() {
                    entry.glyph_name = label.glyph_name.clone();
                }
            }
        }
        self.derive_names();
    }

    /// Fill in `glyph_name` from `unicode` (GF nice-names, uniXXXX fallback)
    /// wherever it is missing.
    pub fn derive_names(&mut self) {
        for entry in &mut self.glyphs {
            if entry.glyph_name.is_some() {
                continue;
            }
            if let Some(ch) = entry.codepoint() {
                entry.glyph_name = Some(glyph_name_for(ch));
            }
        }
    }
}

/// External labels file: `{ "glyph_0001": { "unicode": "U+0061" }, ... }`.
#[derive(Debug, Deserialize)]
pub struct Labels(pub HashMap<String, LabelEntry>);

#[derive(Debug, Deserialize)]
pub struct LabelEntry {
    pub unicode: Option<String>,
    #[serde(default)]
    pub glyph_name: Option<String>,
}

impl Labels {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let labels: Labels = serde_json::from_str(&data)?;
        Ok(labels)
    }
}

/// Production glyph name for a codepoint: the GF Latin Core nice-name when
/// we know it, otherwise AGL-style uniXXXX / uXXXXX.
pub fn glyph_name_for(ch: char) -> String {
    let cp = ch as u32;
    if let Some(name) = gf_latin_core::name_for_codepoint(cp) {
        return name.to_string();
    }
    if cp <= 0xFFFF {
        format!("uni{cp:04X}")
    } else {
        format!("u{cp:05X}")
    }
}

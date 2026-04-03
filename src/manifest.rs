/// Types for deserializing the manifest.json produced by img2glyph.
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct BoundingBox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Deserialize)]
pub struct GlyphEntry {
    pub id: String,
    /// Path to the extracted glyph PNG, relative to the manifest file.
    pub file: PathBuf,
    pub bbox: BoundingBox,
    pub area_px: u64,
    pub row: usize,
    pub col: usize,
    pub unicode: Option<String>,
    pub glyph_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub glyphs: Vec<GlyphEntry>,
}

impl Manifest {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let manifest: Manifest = serde_json::from_str(&data)?;
        Ok(manifest)
    }
}

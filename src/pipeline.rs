use crate::manifest::Manifest;
use crate::ufo_builder;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PipelineConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    /// If Some, use this directory for glyph PNGs and manifest.json.
    /// If None, use a temporary directory that is cleaned up on exit.
    pub glyph_dir: Option<PathBuf>,
    pub family_name: String,
    pub style_name: String,
    pub upm: u32,
    pub ascender: i32,
    pub descender: i32,
    pub x_height: i32,
    pub cap_height: i32,
    pub accuracy: f64,
    pub smooth_iterations: usize,
    pub alphamax: f64,
    pub grid: i32,
    pub min_area: u32,
    pub max_area: u32,
    pub verbose: bool,
}

pub fn run(config: PipelineConfig) -> Result<()> {
    // Resolve the glyph directory — either the user-supplied path or a tempdir.
    let _temp_dir_holder;
    let glyph_dir: PathBuf = match &config.glyph_dir {
        Some(dir) => {
            std::fs::create_dir_all(dir)?;
            dir.clone()
        }
        None => {
            _temp_dir_holder = tempfile::tempdir()?;
            _temp_dir_holder.path().to_path_buf()
        }
    };

    let manifest_path = glyph_dir.join("manifest.json");

    // Skip segmentation if a manifest already exists in the glyph directory.
    if manifest_path.exists() {
        if config.verbose {
            eprintln!(
                "pipeline: manifest found at {:?}, skipping segmentation",
                manifest_path
            );
        }
    } else {
        step_segment(&config.input, &glyph_dir, config.min_area, config.max_area, config.verbose)?;
    }

    let manifest = Manifest::load(&manifest_path)
        .with_context(|| format!("Failed to load manifest at {:?}", manifest_path))?;

    if config.verbose {
        eprintln!(
            "pipeline: found {} glyphs in manifest",
            manifest.glyphs.len()
        );
    }

    ufo_builder::build(&config, &manifest, &glyph_dir)?;

    Ok(())
}

/// Run `img2glyph process <input> --output <glyph_dir>` as a subprocess.
fn step_segment(input: &Path, glyph_dir: &Path, min_area: u32, max_area: u32, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("pipeline: segmenting {:?} → {:?}", input, glyph_dir);
    }

    let status = Command::new("img2glyph")
        .arg("process")
        .arg(input)
        .arg("--output")
        .arg(glyph_dir)
        .arg("--min-area")
        .arg(min_area.to_string())
        .arg("--max-area")
        .arg(max_area.to_string())
        .status()
        .with_context(|| {
            "Failed to run `img2glyph` — is it installed and on PATH?\n\
             Install it with: cargo install --git https://github.com/eliheuer/img2glyph"
        })?;

    if !status.success() {
        anyhow::bail!("`img2glyph process` exited with status {}", status);
    }

    Ok(())
}

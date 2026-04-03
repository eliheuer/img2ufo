mod manifest;
mod pipeline;
mod ufo_builder;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Convert a type specimen image to a Google Fonts-compliant UFO font source.
///
/// Pipeline:
///   1. Segment glyphs from the input image (img2glyph)
///   2. Trace each glyph PNG to bezier outlines (img2bez)
///   3. Assemble a UFO font source with Google Fonts-compliant metadata
///
/// Use --glyph-dir to save or reuse intermediate glyph PNGs and manifest.json.
/// If the directory already contains a manifest.json, segmentation is skipped.
#[derive(Parser, Debug)]
#[command(name = "img2ufo", version, about)]
struct Args {
    /// Input type specimen image (PNG, JPEG, or BMP)
    #[arg(short, long)]
    input: PathBuf,

    /// Output UFO directory path (e.g. MyFont-Regular.ufo)
    #[arg(short, long)]
    output: PathBuf,

    /// Directory for intermediate glyph PNGs and manifest.json.
    /// If omitted, a temporary directory is used and cleaned up automatically.
    /// If the directory already contains a manifest.json, segmentation is skipped.
    #[arg(long)]
    glyph_dir: Option<PathBuf>,

    /// Family name for the font
    #[arg(long, default_value = "Untitled")]
    family_name: String,

    /// Style name (e.g. Regular, Bold, Italic)
    #[arg(long, default_value = "Regular")]
    style_name: String,

    /// Units per em
    #[arg(long, default_value_t = 1000)]
    upm: u32,

    /// Ascender value in font units
    #[arg(long, default_value_t = 800)]
    ascender: i32,

    /// Descender value in font units (typically negative)
    #[arg(long, default_value_t = -200)]
    descender: i32,

    /// x-height in font units
    #[arg(long, default_value_t = 500)]
    x_height: i32,

    /// Cap-height in font units
    #[arg(long, default_value_t = 700)]
    cap_height: i32,

    /// Curve-fitting accuracy for bezier tracing (lower = more accurate, more points)
    #[arg(long, default_value_t = 2.0)]
    accuracy: f64,

    /// Polyline smoothing iterations before curve fitting (0–3; >1 blurs corners)
    #[arg(long, default_value_t = 1)]
    smooth_iterations: usize,

    /// Corner detection threshold (0.0 = all corners, 1.34 = no corners; 0.8 default)
    #[arg(long, default_value_t = 0.80)]
    alphamax: f64,

    /// Coordinate snapping grid (0 = off)
    #[arg(long, default_value_t = 0)]
    grid: i32,

    /// Minimum glyph area in pixels — raise to filter scan noise
    #[arg(long, default_value_t = 200)]
    min_area: u32,

    /// Maximum glyph area in pixels — lower to exclude large page elements
    #[arg(long, default_value_t = 50000)]
    max_area: u32,

    /// Verbosity
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.verbose {
        eprintln!("img2ufo: processing {:?}", args.input);
    }

    let config = pipeline::PipelineConfig {
        input: args.input,
        output: args.output,
        glyph_dir: args.glyph_dir,
        family_name: args.family_name,
        style_name: args.style_name,
        upm: args.upm,
        ascender: args.ascender,
        descender: args.descender,
        x_height: args.x_height,
        cap_height: args.cap_height,
        accuracy: args.accuracy,
        smooth_iterations: args.smooth_iterations,
        alphamax: args.alphamax,
        grid: args.grid,
        min_area: args.min_area,
        max_area: args.max_area,
        verbose: args.verbose,
    };

    pipeline::run(config)?;

    if args.verbose {
        eprintln!("img2ufo: done");
    }

    Ok(())
}

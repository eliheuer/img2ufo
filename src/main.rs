mod composites;
mod gasp;
mod gf_latin_core;
mod manifest;
mod pipeline;
mod qa;
mod repo_emit;
mod spacing;
mod ufo_builder;

use anyhow::Result;
use clap::Parser;
use img2bez::Profile;
use std::path::PathBuf;

/// Convert a type specimen image to a Google Fonts-compliant UFO font source.
///
/// Simplest use: `img2ufo scan.png` — the UFO (plus completion worklist,
/// compiled TTF, and fontspector report) lands next to the image. If a
/// manifest.json / labels.json sit beside the image (an img2glyph output
/// directory), they are picked up automatically and segmentation is skipped.
///
/// Pipeline:
///   1. Segment glyphs from the input image (img2glyph)
///   2. Label glyph crops with codepoints (--labels)
///   3. Trace each glyph PNG to bezier outlines (img2bez)
///   4. Assemble a UFO with GF-compliant metadata, then auto-compose the
///      missing Latin Core accented glyphs (+ optional repo scaffold)
///   5. Compile (fontc) and gate on fontspector's googlefonts profile
///
/// Use --glyph-dir to save or reuse intermediate glyph PNGs and manifest.json.
/// If the directory already contains a manifest.json, segmentation is skipped.
#[derive(Parser, Debug)]
#[command(name = "img2ufo", version, about)]
struct Args {
    /// Input type specimen image (PNG, JPEG, or BMP)
    #[arg(value_name = "IMAGE", required_unless_present = "input")]
    image: Option<PathBuf>,

    /// Input image (legacy flag form; same as the positional IMAGE)
    #[arg(short, long, conflicts_with = "image")]
    input: Option<PathBuf>,

    /// Output UFO directory path (e.g. MyFont-Regular.ufo).
    /// Default: <image dir>/<Family>-<Style>.ufo.
    /// Not needed with --emit-repo (the UFO goes into <repo>/sources/).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Directory for intermediate glyph PNGs and manifest.json.
    /// If omitted, a temporary directory is used and cleaned up automatically.
    /// If the directory already contains a manifest.json, segmentation is skipped.
    #[arg(long)]
    glyph_dir: Option<PathBuf>,

    /// Labels file: JSON mapping glyph ids to codepoints,
    /// e.g. {"glyph_0001": {"unicode": "U+0061"}}. Unlabeled glyphs are
    /// included with production names but excluded from the cmap.
    #[arg(long)]
    labels: Option<PathBuf>,

    /// Family name for the font.
    /// Default: derived from the image's directory name (e.g.
    /// scans/garamond-poster/scan.png -> "Garamond Poster").
    #[arg(long, alias = "family-name")]
    family: Option<String>,

    /// Style name (e.g. Regular, Bold, Italic)
    #[arg(long, default_value = "Regular")]
    style: String,

    /// Designer name (name ID 9)
    #[arg(long, default_value = "Unknown")]
    designer: String,

    /// Upstream repository URL used in the GF copyright string
    /// "Copyright {year} The {family} Project Authors ({git_url})".
    /// Default: https://github.com/eliheuer/<family-slug>.
    #[arg(long)]
    git_url: Option<String>,

    /// Units per em (1024 = power-of-2 grid)
    #[arg(long, default_value_t = 1024)]
    upm: u32,

    /// Ascender value in font units
    #[arg(long, default_value_t = 832)]
    ascender: i32,

    /// Descender value in font units (typically negative)
    #[arg(long, default_value_t = -256)]
    descender: i32,

    /// x-height in font units
    #[arg(long, default_value_t = 576)]
    x_height: i32,

    /// Cap-height in font units
    #[arg(long, default_value_t = 768)]
    cap_height: i32,

    /// img2bez source profile: wild, clean, or photo
    #[arg(long, default_value = "wild")]
    profile: String,

    /// Curve-fitting accuracy override in font units
    /// (default: the profile's; lower = more points)
    #[arg(long)]
    accuracy: Option<f64>,

    /// Coordinate snapping grid (2 = even integers for power-of-2 grid; 0 = off)
    #[arg(long, default_value_t = 2)]
    grid: i32,

    /// Minimum glyph area in pixels — raise to filter scan noise
    #[arg(long, default_value_t = 200)]
    min_area: u32,

    /// Maximum glyph area in pixels — lower to exclude large page elements
    #[arg(long, default_value_t = 50000)]
    max_area: u32,

    /// Skip the compile + fontspector QA gate
    #[arg(long)]
    no_qa: bool,

    /// Emit a Google Fonts upstream repo scaffold at this directory
    /// (sources/<ufo> + config.yaml, OFL.txt, README.md, Makefile, ...)
    #[arg(long)]
    emit_repo: Option<PathBuf>,

    /// Verbosity
    #[arg(short, long)]
    verbose: bool,
}

/// Default family name from the specimen's directory (fallback: file stem):
/// non-alphanumerics become spaces, words are title-cased.
fn derive_family(input: &std::path::Path) -> String {
    let source = input
        .parent()
        .and_then(|p| p.file_name())
        .or_else(|| input.file_stem())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = source
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if name.is_empty() {
        "Untitled Revival".to_string()
    } else {
        name
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let profile: Profile = args
        .profile
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;

    let input = args
        .image
        .or(args.input)
        .expect("clap enforces IMAGE or --input");
    let family = args.family.unwrap_or_else(|| derive_family(&input));

    let mut config = pipeline::PipelineConfig {
        input,
        output: args.output,
        glyph_dir: args.glyph_dir,
        labels: args.labels,
        family,
        style: args.style,
        designer: args.designer,
        git_url: args.git_url,
        upm: args.upm,
        ascender: args.ascender,
        descender: args.descender,
        x_height: args.x_height,
        cap_height: args.cap_height,
        profile,
        accuracy: args.accuracy,
        grid: args.grid,
        min_area: args.min_area,
        max_area: args.max_area,
        no_qa: args.no_qa,
        emit_repo: args.emit_repo,
        verbose: args.verbose,
        copyright: String::new(),
    };
    config.copyright = pipeline::gf_copyright(
        pipeline::current_year(),
        &config.family,
        &config.effective_git_url(),
    );

    pipeline::run(config)
}

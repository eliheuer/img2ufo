use crate::manifest::{GlyphEntry, Manifest};
use crate::pipeline::PipelineConfig;
use anyhow::{Context, Result};
use img2bez::{trace, TracingConfig};
use kurbo::PathEl;
use norad::{Contour, ContourPoint, Font, FontInfo, Glyph, PointType};
use norad::fontinfo::NonNegativeIntegerOrFloat;
use std::path::Path;

/// Build a Google Fonts-compliant UFO from the segmented glyph PNGs.
pub fn build(config: &PipelineConfig, manifest: &Manifest, glyph_dir: &Path) -> Result<()> {
    let mut font = Font::new();

    apply_font_info(&mut font.font_info, config);

    for entry in &manifest.glyphs {
        // Skip unlabeled entries (noise/blank from segmentation).
        if entry.glyph_name.is_none() && entry.unicode.is_none() {
            if config.verbose {
                eprintln!("ufo_builder: skipping unlabeled {:?}", entry.file);
            }
            continue;
        }

        let png_path = glyph_dir.join(&entry.file);

        match trace_and_add_glyph(&mut font, entry, &png_path, config) {
            Ok(_) => {
                if config.verbose {
                    eprintln!(
                        "ufo_builder: traced {:?} → {}",
                        entry.file,
                        entry.glyph_name.as_deref().unwrap_or("(unnamed)")
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "ufo_builder: warning: skipping {:?}: {}",
                    entry.file, e
                );
            }
        }
    }

    font.save(&config.output)
        .with_context(|| format!("Failed to write UFO to {:?}", config.output))?;

    Ok(())
}

fn trace_and_add_glyph(
    font: &mut Font,
    entry: &GlyphEntry,
    png_path: &Path,
    config: &PipelineConfig,
) -> Result<()> {
    let glyph_name = entry
        .glyph_name
        .as_deref()
        .unwrap_or(entry.id.as_str());

    let tracing_config = TracingConfig {
        target_height: config.upm as f64,
        grid: config.grid,
        fit_accuracy: config.accuracy,
        smooth_iterations: config.smooth_iterations,
        alphamax: config.alphamax,
        y_offset: config.descender as f64,
        ..Default::default()
    };

    let result = trace(png_path, &tracing_config)
        .with_context(|| format!("img2bez tracing failed for {:?}", png_path))?;

    let mut glyph = Glyph::new(glyph_name);
    glyph.width = result.advance_width;

    // Set Unicode codepoint if present.
    if let Some(ref hex) = entry.unicode {
        if let Ok(cp) = u32::from_str_radix(hex.trim_start_matches("U+"), 16) {
            if let Some(ch) = char::from_u32(cp) {
                glyph.codepoints.insert(ch);
            }
        }
    }

    // Convert kurbo bezier paths to norad contours.
    for bez_path in &result.paths {
        if let Some(contour) = kurbo_path_to_norad_contour(bez_path) {
            glyph.contours.push(contour);
        }
    }

    font.default_layer_mut().insert_glyph(glyph);

    Ok(())
}

/// Convert a `kurbo::BezPath` to a closed `norad::Contour`.
///
/// UFO closed contours do NOT use PointType::Move. Instead the first point
/// gets the type of the closing segment (Curve, Line, etc.) and the contour
/// is implicitly cyclic. This matches how norad/img2bez represent closed paths.
fn kurbo_path_to_norad_contour(path: &kurbo::BezPath) -> Option<Contour> {
    let elements = path.elements();
    if elements.is_empty() {
        return None;
    }

    // First element must be MoveTo — grab its position.
    let first = match elements.first()? {
        PathEl::MoveTo(p) => *p,
        _ => return None,
    };

    let mut points: Vec<ContourPoint> = Vec::new();

    for el in elements.iter().skip(1) {
        match *el {
            PathEl::LineTo(p) => {
                points.push(cp(p, PointType::Line));
            }
            PathEl::CurveTo(a, b, p) => {
                points.push(cp(a, PointType::OffCurve));
                points.push(cp(b, PointType::OffCurve));
                points.push(cp(p, PointType::Curve));
            }
            PathEl::QuadTo(a, p) => {
                points.push(cp(a, PointType::OffCurve));
                points.push(cp(p, PointType::QCurve));
            }
            PathEl::ClosePath | PathEl::MoveTo(_) => {}
        }
    }

    if points.is_empty() {
        return None;
    }

    // Determine the closing type from the last segment.
    let closing_type = elements
        .iter()
        .rev()
        .find(|e| !matches!(e, PathEl::ClosePath))
        .map(|e| match e {
            PathEl::CurveTo(..) => PointType::Curve,
            PathEl::QuadTo(..) => PointType::QCurve,
            _ => PointType::Line,
        })
        .unwrap_or(PointType::Line);

    // Remove duplicate closing point if the last on-curve equals the MoveTo.
    let last_oncurve = points.iter().rposition(|p| {
        matches!(p.typ, PointType::Curve | PointType::Line | PointType::QCurve)
    });
    if let Some(idx) = last_oncurve {
        let last = &points[idx];
        if (last.x - first.x).abs() < 0.5 && (last.y - first.y).abs() < 0.5 {
            points.remove(idx);
        }
    }

    // Insert the first point with the closing type (UFO cyclic convention).
    points.insert(0, cp(first, closing_type));

    Some(Contour::new(points, None))
}

#[inline]
fn cp(p: kurbo::Point, typ: PointType) -> ContourPoint {
    ContourPoint::new(p.x, p.y, typ, false, None, None)
}

/// Populate `FontInfo` with Google Fonts-required fields.
fn apply_font_info(info: &mut FontInfo, config: &PipelineConfig) {
    info.family_name = Some(config.family_name.clone());
    info.style_name = Some(config.style_name.clone());

    // Postscript name: "FamilyName-StyleName", spaces removed.
    info.postscript_font_name = Some(format!(
        "{}-{}",
        config.family_name.replace(' ', ""),
        config.style_name.replace(' ', "")
    ));

    // Full name: "Family Style" (used in menus).
    info.postscript_full_name =
        Some(format!("{} {}", config.family_name, config.style_name));

    // Vertical metrics.
    info.units_per_em = NonNegativeIntegerOrFloat::new(config.upm as f64);
    info.ascender = Some(config.ascender as f64);
    info.descender = Some(config.descender as f64);
    info.x_height = Some(config.x_height as f64);
    info.cap_height = Some(config.cap_height as f64);

    // Google Fonts requires these OS/2 values.
    info.open_type_os2_typo_ascender = Some(config.ascender);
    info.open_type_os2_typo_descender = Some(config.descender);
    info.open_type_os2_typo_line_gap = Some(0);
    info.open_type_os2_win_ascent = Some(config.ascender as u32);
    info.open_type_os2_win_descent = Some(config.descender.unsigned_abs());

    // hhea metrics (must match typo for GF compliance).
    info.open_type_hhea_ascender = Some(config.ascender);
    info.open_type_hhea_descender = Some(config.descender);
    info.open_type_hhea_line_gap = Some(0);

    // Minimal head flags required by GF spec.
    // Bit 0: baseline at y=0. Bit 3: force ppem to integer.
    info.open_type_head_flags = Some(vec![0, 3]);

    // Copyright placeholder — user should update before submission.
    info.copyright = Some(format!(
        "Copyright 2026 The {} Authors",
        config.family_name
    ));

    // Version string (Google Fonts format: "Version X.YYY").
    info.open_type_name_version = Some("Version 0.001".to_string());
}

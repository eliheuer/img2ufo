use crate::gf_latin_core;
use crate::manifest::{GlyphEntry, Manifest};
use crate::pipeline::PipelineConfig;
use anyhow::{Context, Result};
use img2bez::{trace, TracingConfig};
use kurbo::PathEl;
use norad::{Contour, ContourPoint, Font, FontInfo, Glyph, PointType};
use norad::fontinfo::NonNegativeIntegerOrFloat;
use plist::Value;
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Specimen-level metrics: uniform scale + per-row baseline
// ============================================================================

/// Metrics derived from the specimen layout for uniform glyph scaling.
struct SpecimenMetrics {
    /// Pixels-to-font-units scale factor (same for all glyphs).
    uniform_scale: f64,
    /// Baseline y-coordinate (in source image pixels, y-down) per row.
    baselines: HashMap<usize, f64>,
    /// Padding used when cropping (needed to compute crop bounds).
    padding: u32,
}

impl SpecimenMetrics {
    /// Analyze the manifest to compute a uniform scale and per-row baselines.
    fn from_manifest(manifest: &Manifest, cap_height: f64, padding: u32) -> Self {
        // Find uppercase letters (A-Z) to determine scale and baselines.
        let uppercase: Vec<&GlyphEntry> = manifest
            .glyphs
            .iter()
            .filter(|g| {
                g.unicode.as_deref().map_or(false, |u| {
                    let cp = u32::from_str_radix(u.trim_start_matches("U+"), 16).unwrap_or(0);
                    (0x0041..=0x005A).contains(&cp) // A-Z
                })
            })
            .collect();

        // Uniform scale: tallest uppercase height → cap_height.
        // Prefer H (no overshoot) if available, else use the median uppercase height.
        let reference_height = uppercase
            .iter()
            .find(|g| g.glyph_name.as_deref() == Some("H"))
            .map(|g| g.bbox.h as f64)
            .unwrap_or_else(|| {
                let mut heights: Vec<f64> = uppercase.iter().map(|g| g.bbox.h as f64).collect();
                heights.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if heights.is_empty() {
                    // No uppercase — fallback to median of all labeled glyphs.
                    let mut all: Vec<f64> = manifest
                        .glyphs
                        .iter()
                        .filter(|g| g.glyph_name.is_some())
                        .map(|g| g.bbox.h as f64)
                        .collect();
                    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    all.get(all.len() / 2).copied().unwrap_or(200.0)
                } else {
                    heights[heights.len() / 2]
                }
            });

        let uniform_scale = cap_height / reference_height;

        // Per-row baselines: average bottom edge of uppercase letters in each row.
        let mut row_uc_bottoms: HashMap<usize, Vec<f64>> = HashMap::new();
        for g in &uppercase {
            row_uc_bottoms
                .entry(g.row)
                .or_default()
                .push((g.bbox.y + g.bbox.h) as f64);
        }

        let mut baselines: HashMap<usize, f64> = HashMap::new();
        for (row, bottoms) in &row_uc_bottoms {
            let avg = bottoms.iter().sum::<f64>() / bottoms.len() as f64;
            baselines.insert(*row, avg);
        }

        // For rows without uppercase, try to infer from non-descender glyphs
        // or fall back to the nearest row that has a baseline.
        let all_rows: Vec<usize> = manifest.glyphs.iter().map(|g| g.row).collect();
        let max_row = all_rows.iter().max().copied().unwrap_or(0);

        for row in 0..=max_row {
            if baselines.contains_key(&row) {
                continue;
            }
            // Use non-descender glyphs: their bottom edge ≈ baseline.
            let non_descender_bottoms: Vec<f64> = manifest
                .glyphs
                .iter()
                .filter(|g| g.row == row && g.glyph_name.is_some())
                .filter(|g| {
                    // Exclude known descender glyphs
                    let name = g.glyph_name.as_deref().unwrap_or("");
                    !matches!(name, "g" | "j" | "p" | "q" | "y" | "Q" | "J")
                })
                .map(|g| (g.bbox.y + g.bbox.h) as f64)
                .collect();

            if !non_descender_bottoms.is_empty() {
                let median_idx = non_descender_bottoms.len() / 2;
                let mut sorted = non_descender_bottoms.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                baselines.insert(row, sorted[median_idx]);
            } else {
                // Last resort: find nearest row with a baseline.
                if let Some(nearest) = baselines.keys().min_by_key(|&&r| (r as isize - row as isize).unsigned_abs()) {
                    baselines.insert(row, baselines[nearest]);
                }
            }
        }

        Self {
            uniform_scale,
            baselines,
            padding,
        }
    }

    /// Compute per-glyph tracing parameters (target_height, y_offset).
    fn glyph_params(&self, entry: &GlyphEntry, source_height: u32) -> (f64, f64) {
        let padding = self.padding;

        // Reconstruct the crop bounds in source image coordinates.
        let crop_top = (entry.bbox.y).saturating_sub(padding) as f64;
        let crop_bottom = ((entry.bbox.y + entry.bbox.h + padding) as u32).min(source_height) as f64;
        let crop_h = crop_bottom - crop_top;

        // Target height: scale the crop uniformly.
        let target_height = crop_h * self.uniform_scale;

        // Baseline for this glyph's row.
        let baseline_y = self
            .baselines
            .get(&entry.row)
            .copied()
            .unwrap_or(crop_bottom); // fallback: bottom of crop = baseline

        // y_offset: the bottom of the crop in font units should place the baseline at y=0.
        // In img2bez, bottom of image = y_offset, top = y_offset + target_height.
        // The baseline is (crop_bottom - baseline_y) pixels from the bottom of the crop.
        // After scaling: (crop_bottom - baseline_y) * uniform_scale + y_offset = 0
        let y_offset = -((crop_bottom - baseline_y) * self.uniform_scale);

        (target_height, y_offset)
    }
}

/// Build a Google Fonts-compliant UFO from the segmented glyph PNGs.
pub fn build(config: &PipelineConfig, manifest: &Manifest, glyph_dir: &Path) -> Result<()> {
    let mut font = Font::new();

    apply_font_info(&mut font.font_info, config);

    // Add required .notdef and space glyphs.
    add_notdef(&mut font, config);
    add_space(&mut font, config);

    // Compute uniform scale and per-row baselines from the specimen layout.
    let specimen = SpecimenMetrics::from_manifest(manifest, config.cap_height as f64, 10);
    if config.verbose {
        eprintln!(
            "ufo_builder: uniform scale = {:.3} units/px (cap ref → {} units)",
            specimen.uniform_scale, config.cap_height
        );
        for (row, bl) in &specimen.baselines {
            eprintln!("ufo_builder: row {} baseline at y={:.0}px", row, bl);
        }
    }

    // Read source image dimensions for crop-bound clamping.
    let source_height = manifest
        .glyphs
        .iter()
        .map(|g| g.bbox.y + g.bbox.h + 20)
        .max()
        .unwrap_or(2000);

    // Track glyph order for lib.plist.
    let mut glyph_order: Vec<String> = vec![".notdef".into(), "space".into()];

    for entry in &manifest.glyphs {
        // Skip unlabeled entries (noise/blank from segmentation).
        if entry.glyph_name.is_none() && entry.unicode.is_none() {
            if config.verbose {
                eprintln!("ufo_builder: skipping unlabeled {:?}", entry.file);
            }
            continue;
        }

        let png_path = glyph_dir.join(&entry.file);
        let (target_height, y_offset) = specimen.glyph_params(entry, source_height);

        match trace_and_add_glyph(&mut font, entry, &png_path, config, target_height, y_offset) {
            Ok(name) => {
                glyph_order.push(name.clone());
                if config.verbose {
                    eprintln!("ufo_builder: traced {:?} → {}", entry.file, name);
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

    // Add empty placeholder glyphs for all GF Latin Core codepoints
    // not already present from the traced specimen.
    let existing: std::collections::HashSet<String> = glyph_order.iter().cloned().collect();
    let default_width = config.upm as f64 / 2.0;
    for &(cp, name) in gf_latin_core::GLYPHSET {
        if !existing.contains(name) {
            let mut glyph = Glyph::new(name);
            glyph.width = default_width;
            if let Some(ch) = char::from_u32(cp) {
                glyph.codepoints.insert(ch);
            }
            font.default_layer_mut().insert_glyph(glyph);
            glyph_order.push(name.to_string());
        }
    }

    // lib.plist: glyph order.
    apply_lib(&mut font, &glyph_order);

    font.save(&config.output)
        .with_context(|| format!("Failed to write UFO to {:?}", config.output))?;

    Ok(())
}

fn trace_and_add_glyph(
    font: &mut Font,
    entry: &GlyphEntry,
    png_path: &Path,
    config: &PipelineConfig,
    target_height: f64,
    y_offset: f64,
) -> Result<String> {
    let glyph_name = entry
        .glyph_name
        .as_deref()
        .unwrap_or(entry.id.as_str());

    let tracing_config = TracingConfig {
        target_height,
        y_offset,
        grid: config.grid,
        fit_accuracy: config.accuracy,
        smooth_iterations: config.smooth_iterations,
        alphamax: config.alphamax,
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

    // Store the image→font transform so editors can place the source
    // image exactly where the trace came from. The transform is:
    //   scale = target_height / image_pixel_height
    //   font_x = pixel_x * scale + shift_x
    //   font_y = pixel_y * scale + shift_y
    // where shift = reposition_shift from img2bez.
    {
        let (shift_x, shift_y) = result.reposition_shift;
        let img = image::open(png_path).ok();
        let img_h = img.as_ref().map(|i| i.height()).unwrap_or(1) as f64;
        let scale = target_height / img_h;
        glyph.lib.insert(
            "com.img2ufo.imageScale".into(),
            Value::Real(scale),
        );
        glyph.lib.insert(
            "com.img2ufo.imageOffsetX".into(),
            Value::Real(shift_x),
        );
        glyph.lib.insert(
            "com.img2ufo.imageOffsetY".into(),
            Value::Real(shift_y + y_offset),
        );
    }

    // Convert kurbo bezier paths to norad contours.
    for bez_path in &result.paths {
        if let Some(contour) = kurbo_path_to_norad_contour(bez_path) {
            glyph.contours.push(contour);
        }
    }

    font.default_layer_mut().insert_glyph(glyph);

    Ok(glyph_name.to_string())
}

/// Add a .notdef glyph with a standard empty rectangle (required by OpenType spec).
fn add_notdef(font: &mut Font, config: &PipelineConfig) {
    let upm = config.upm as f64;
    let w = upm * 0.5;
    let asc = config.ascender as f64;
    let desc = config.descender as f64;
    let stroke = upm * 0.05;

    let mut glyph = Glyph::new(".notdef");
    glyph.width = w;

    // Outer rectangle
    let outer = Contour::new(
        vec![
            cp(kurbo::Point::new(stroke, desc + stroke), PointType::Line),
            cp(kurbo::Point::new(w - stroke, desc + stroke), PointType::Line),
            cp(kurbo::Point::new(w - stroke, asc - stroke), PointType::Line),
            cp(kurbo::Point::new(stroke, asc - stroke), PointType::Line),
        ],
        None,
    );
    // Inner rectangle (counter — opposite winding)
    let inner = Contour::new(
        vec![
            cp(kurbo::Point::new(stroke * 2.0, desc + stroke * 2.0), PointType::Line),
            cp(kurbo::Point::new(stroke * 2.0, asc - stroke * 2.0), PointType::Line),
            cp(kurbo::Point::new(w - stroke * 2.0, asc - stroke * 2.0), PointType::Line),
            cp(kurbo::Point::new(w - stroke * 2.0, desc + stroke * 2.0), PointType::Line),
        ],
        None,
    );
    glyph.contours.push(outer);
    glyph.contours.push(inner);

    font.default_layer_mut().insert_glyph(glyph);
}

/// Add a space glyph (required for any text font).
fn add_space(font: &mut Font, config: &PipelineConfig) {
    let mut glyph = Glyph::new("space");
    glyph.width = config.upm as f64 / 4.0;
    glyph.codepoints.insert(' ');
    font.default_layer_mut().insert_glyph(glyph);
}

/// Populate lib.plist with Google Fonts-required keys.
fn apply_lib(font: &mut Font, glyph_order: &[String]) {
    // public.glyphOrder — controls glyph ordering in compiled font.
    let order: Vec<Value> = glyph_order.iter().map(|s| Value::String(s.clone())).collect();
    font.lib.insert("public.glyphOrder".into(), Value::Array(order));

    // public.skipExportGlyphs — empty, but present for tooling compat.
    font.lib.insert("public.skipExportGlyphs".into(), Value::Array(vec![]));
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
    let ps_family = config.family_name.replace(' ', "");
    let ps_style = config.style_name.replace(' ', "");
    info.postscript_font_name = Some(format!("{}-{}", ps_family, ps_style));
    info.postscript_full_name =
        Some(format!("{} {}", config.family_name, config.style_name));

    // Vertical metrics.
    info.units_per_em = NonNegativeIntegerOrFloat::new(config.upm as f64);
    info.ascender = Some(config.ascender as f64);
    info.descender = Some(config.descender as f64);
    info.x_height = Some(config.x_height as f64);
    info.cap_height = Some(config.cap_height as f64);

    // Google Fonts vertical metrics strategy:
    // typo and hhea must match, lineGap must be 0, fsSelection bit 7 set.
    // Sum of ascender + abs(descender) must be >= 1200 for the hhea check.
    // With 1024 UPM: use typo ascender = UPM (1024), descender as-is (-256).
    // Sum = 1024 + 256 = 1280 >= 1200.
    let typo_asc = (config.upm as i32).max(config.ascender);
    let typo_desc = config.descender;
    info.open_type_os2_typo_ascender = Some(typo_asc);
    info.open_type_os2_typo_descender = Some(typo_desc);
    info.open_type_os2_typo_line_gap = Some(0);
    // winAscent/winDescent should cover the font's actual bounding box.
    // Use generous values to account for overshoots and accented characters.
    info.open_type_os2_win_ascent = Some(typo_asc.max(config.ascender + 128) as u32);
    info.open_type_os2_win_descent = Some(typo_desc.unsigned_abs().max(config.descender.unsigned_abs() + 128));
    info.open_type_hhea_ascender = Some(typo_asc);
    info.open_type_hhea_descender = Some(typo_desc);
    info.open_type_hhea_line_gap = Some(0);

    // fsSelection bit 7 = USE_TYPO_METRICS (required by Google Fonts).
    // Bits 0, 5, 6 are set automatically by the compiler from style name.
    info.open_type_os2_selection = Some(vec![7]);

    // fsType = 0 (Installable embedding, required by Google Fonts).
    info.open_type_os2_type = Some(vec![]);

    // Head flags: bit 0 (baseline at y=0), bit 3 (force ppem to integer).
    info.open_type_head_flags = Some(vec![0, 3]);

    // Copyright (Google Fonts format).
    info.copyright = Some(format!(
        "Copyright 2026 The {} Project Authors (https://github.com/example/{})",
        config.family_name,
        config.family_name.to_lowercase().replace(' ', "-")
    ));

    // Version string (Google Fonts requires >= 1.000).
    info.open_type_name_version = Some("Version 1.000".to_string());
    info.version_major = Some(1);
    info.version_minor = Some(0);

    // License (SIL OFL, required by Google Fonts).
    info.open_type_name_license = Some(
        "This Font Software is licensed under the SIL Open Font License, Version 1.1. \
         This license is available with a FAQ at: https://openfontlicense.org"
            .to_string(),
    );
    info.open_type_name_license_url = Some("https://openfontlicense.org".to_string());

    // Designer name (required by Google Fonts, nameId=9).
    info.open_type_name_designer = Some("Unknown".to_string());

    // Vendor ID (use "NONE" as placeholder).
    info.open_type_os2_vendor_id = Some("NONE".to_string());
}

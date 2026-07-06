//! End-to-end test for the auto-composition stage: a tiny synthetic
//! specimen (drawn in Rust via the `image` crate — no external tools) is
//! traced by the real binary, and the output UFO must contain anchored
//! composites, derived glyphs, and the completion worklist.
//!
//! Dataset: A (rect), i (stem + tittle), acutecomb (slanted bar).
//! Expected: acute (spacing alias) + dotlessi derived; Aacute + iacute
//! composed; everything else honestly reported missing.

use assert_cmd::Command;
use img2bez::image::{GrayImage, Luma};

const WHITE: Luma<u8> = Luma([255u8]);
const BLACK: Luma<u8> = Luma([0u8]);

fn blank(w: u32, h: u32) -> GrayImage {
    GrayImage::from_pixel(w, h, WHITE)
}

fn fill_rect(img: &mut GrayImage, x0: u32, y0: u32, x1: u32, y1: u32) {
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, BLACK);
        }
    }
}

/// Slanted thick bar from bottom-left to top-right (an acute).
fn acute_bar(img: &mut GrayImage) {
    let (w, h) = img.dimensions();
    for y in 0..h {
        // x center runs right-to-left as y grows (y is top-down).
        let t = y as f64 / h as f64;
        let cx = (w as f64) * (0.8 - 0.6 * t);
        for x in 0..w {
            if (x as f64 - cx).abs() <= w as f64 * 0.18 {
                img.put_pixel(x, y, BLACK);
            }
        }
    }
}

#[test]
fn composes_accents_from_a_traced_specimen() {
    let dir = tempfile::tempdir().unwrap();
    let glyph_dir = dir.path().join("glyphs");
    std::fs::create_dir_all(&glyph_dir).unwrap();

    // --- crops (PNG resolution is independent of the manifest bbox) ---
    // A: 200x200 crop, plain filled square.
    let mut a = blank(200, 200);
    fill_rect(&mut a, 10, 10, 190, 190);
    a.save(glyph_dir.join("glyph_0001.png")).unwrap();

    // i: stem (bottom 60%) + tittle (top 15%), separated.
    let mut i = blank(60, 160);
    fill_rect(&mut i, 15, 64, 45, 160); // stem
    fill_rect(&mut i, 15, 0, 45, 24); // tittle
    i.save(glyph_dir.join("glyph_0002.png")).unwrap();

    // acutecomb: slanted bar.
    let mut acc = blank(80, 70);
    acute_bar(&mut acc);
    acc.save(glyph_dir.join("glyph_0003.png")).unwrap();

    // --- manifest: one row, baseline at sheet y=150 (A and i bottoms) ---
    let manifest = serde_json::json!({
        "glyphs": [
            {"id": "glyph_0001", "file": "glyph_0001.png",
             "bbox": {"x": 10, "y": 50, "w": 100, "h": 100},
             "area_px": 10000, "row": 0, "col": 0,
             "unicode": null, "glyph_name": null},
            {"id": "glyph_0002", "file": "glyph_0002.png",
             "bbox": {"x": 130, "y": 70, "w": 30, "h": 80},
             "area_px": 1800, "row": 0, "col": 1,
             "unicode": null, "glyph_name": null},
            {"id": "glyph_0003", "file": "glyph_0003.png",
             "bbox": {"x": 180, "y": 40, "w": 40, "h": 35},
             "area_px": 700, "row": 0, "col": 2,
             "unicode": null, "glyph_name": null}
        ]
    });
    std::fs::write(
        glyph_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let labels = serde_json::json!({
        "glyph_0001": {"unicode": "U+0041"},
        "glyph_0002": {"unicode": "U+0069"},
        "glyph_0003": {"unicode": "U+0301"}
    });
    let labels_path = dir.path().join("labels.json");
    std::fs::write(&labels_path, serde_json::to_string(&labels).unwrap()).unwrap();

    // --- run the real pipeline (no QA: fontc/fontspector not required) ---
    let ufo = dir.path().join("Test-Regular.ufo");
    let assert = Command::cargo_bin("img2ufo")
        .unwrap()
        .arg("-i")
        .arg(dir.path().join("unused.png")) // segmentation skipped: manifest exists
        .arg("--glyph-dir")
        .arg(&glyph_dir)
        .arg("--labels")
        .arg(&labels_path)
        .arg("--family")
        .arg("Composition Test")
        .arg("-o")
        .arg(&ufo)
        .arg("--no-qa")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();

    // Aacute + iacute composed; acute (spacing) + dotlessi derived.
    assert!(
        stdout.contains("+2 composites"),
        "expected 2 composites in: {stdout}"
    );
    let glif_bodies: Vec<String> = std::fs::read_dir(ufo.join("glyphs"))
        .unwrap()
        .filter_map(|e| std::fs::read_to_string(e.unwrap().path()).ok())
        .collect();
    let containing = |needle: &str| glif_bodies.iter().filter(|b| b.contains(needle)).count();
    // Two composites reference the mark as a component; the spacing alias
    // references it too.
    assert_eq!(containing("base=\"acutecomb\""), 3, "acutecomb consumers");
    assert!(containing("base=\"dotlessi\"") >= 1, "iacute uses dotlessi");
    // Anchors written on base and mark.
    assert!(containing("<anchor") >= 2, "anchors present");

    // Worklist exists and reports honest gaps.
    let worklist: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("Test-Regular-completion.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(worklist["glyphset"], "GF_Latin_Core");
    let traced = worklist["coverage_traced"]["covered"].as_u64().unwrap();
    let after = worklist["coverage_after_composition"]["covered"]
        .as_u64()
        .unwrap();
    // +Aacute +iacute +acute +dotlessi = 4 new encoded glyphs.
    assert_eq!(after, traced + 4, "coverage delta");
    let missing_marks = worklist["missing_marks"].as_array().unwrap();
    assert!(
        missing_marks
            .iter()
            .any(|m| m["name"] == "gravecomb" && !m["unlocks"].as_array().unwrap().is_empty()),
        "gravecomb should be a ranked missing mark"
    );
    // The UFO itself is loadable and the composite carries its codepoint.
    let font = img2bez::norad::Font::load(&ufo).unwrap();
    let aacute = font.default_layer().get_glyph("Aacute").expect("Aacute in UFO");
    assert!(aacute.codepoints.contains('\u{00C1}'));
    assert_eq!(aacute.components.len(), 2);
}

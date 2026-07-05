//! Compile + QA gate: UFO -> TTF (fontc preferred, fontmake fallback),
//! gasp patch, then fontspector's googlefonts profile. The pipeline fails
//! on fontspector FAILs — no silent shipping.

use crate::gasp;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Known fontmake location in the img2bez-data venv (fallback when fontc
/// is not installed).
const FONTMAKE_VENV: &str = "/Users/eli/GH/repos/img2bez-data/.venv/bin/fontmake";

enum Compiler {
    Fontc(PathBuf),
    Fontmake(PathBuf),
}

fn find_compiler() -> Result<Compiler> {
    if let Some(fontc) = which("fontc") {
        return Ok(Compiler::Fontc(fontc));
    }
    let venv_fontmake = PathBuf::from(FONTMAKE_VENV);
    if venv_fontmake.is_file() {
        return Ok(Compiler::Fontmake(venv_fontmake));
    }
    if let Some(fontmake) = which("fontmake") {
        return Ok(Compiler::Fontmake(fontmake));
    }
    bail!(
        "No font compiler found. Install one with `cargo install fontc` \
         or `pip install fontmake`."
    );
}

fn which(name: &str) -> Option<PathBuf> {
    let out = Command::new("which").arg(name).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8(out.stdout).ok()?;
    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Compile the UFO to a TTF and patch in the gasp table.
pub fn compile(ufo: &Path, ttf: &Path, verbose: bool) -> Result<()> {
    if let Some(parent) = ttf.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let compiler = find_compiler()?;
    // Run in a scratch dir: fontc drops a build/ IR directory in the cwd.
    // Paths must be absolute since the compiler runs elsewhere.
    let ufo = ufo.canonicalize()?;
    let ttf = if ttf.is_absolute() {
        ttf.to_path_buf()
    } else {
        std::env::current_dir()?.join(ttf)
    };
    let (ufo, ttf) = (ufo.as_path(), ttf.as_path());
    let scratch = tempfile::tempdir()?;
    let (program, args): (&Path, Vec<&std::ffi::OsStr>) = match &compiler {
        Compiler::Fontc(path) => (
            path.as_path(),
            vec![ufo.as_os_str(), "-o".as_ref(), ttf.as_os_str()],
        ),
        Compiler::Fontmake(path) => (
            path.as_path(),
            vec![
                "-u".as_ref(),
                ufo.as_os_str(),
                "-o".as_ref(),
                "ttf".as_ref(),
                "--output-path".as_ref(),
                ttf.as_os_str(),
            ],
        ),
    };
    if verbose {
        eprintln!("qa: compiling {ufo:?} -> {ttf:?} with {program:?}");
    }
    let output = Command::new(program)
        .args(&args)
        .current_dir(scratch.path())
        .output()
        .with_context(|| format!("Failed to run compiler {program:?}"))?;
    if !output.status.success() {
        bail!(
            "font compile failed ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // gasp table (required by Google Fonts for unhinted fonts).
    gasp::fix_gasp(ttf)?;
    Ok(())
}

/// Parsed fontspector result summary.
pub struct QaSummary {
    pub fails: usize,
    pub warns: usize,
    pub errors: usize,
    /// FAILed check ids -> occurrence count, worst first.
    pub failed_checks: Vec<(String, usize)>,
}

impl QaSummary {
    pub fn passed(&self) -> bool {
        self.fails == 0 && self.errors == 0
    }
}

/// Run `fontspector -p googlefonts --json <report>` on the TTF and parse
/// the report. The raw JSON report is left at `report_path`.
///
/// `exclude_checks`: check ids to skip. Repo-emit mode excludes
/// `googlefonts/repo/dirname_matches_nameid_1`, which assumes the
/// google/fonts PR layout (`ofl/<familyname>/Font.ttf`) and false-positives
/// on the upstream `fonts/ttf/` layout this pipeline emits.
pub fn run_fontspector(
    ttf: &Path,
    report_path: &Path,
    exclude_checks: &[&str],
    verbose: bool,
) -> Result<QaSummary> {
    let Some(fontspector) = which("fontspector") else {
        bail!(
            "fontspector not found on PATH. Install it with \
             `cargo install fontspector` (or skip QA with --no-qa)."
        );
    };
    if verbose {
        eprintln!("qa: running fontspector googlefonts profile on {ttf:?}");
    }
    let mut cmd = Command::new(fontspector);
    cmd.arg("-p")
        .arg("googlefonts")
        .arg("--json")
        .arg(report_path);
    for check in exclude_checks {
        cmd.arg("-x").arg(check);
    }
    let output = cmd.arg(ttf).output().context("Failed to run fontspector")?;
    // fontspector exits nonzero when checks FAIL; the report is still
    // written. Only bail if the report is missing.
    if !report_path.is_file() {
        bail!(
            "fontspector produced no report ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let data = std::fs::read_to_string(report_path)?;
    let json: serde_json::Value = serde_json::from_str(&data)
        .context("Cannot parse fontspector JSON report")?;
    let mut summary = summarize(&json);
    // Prefer the report's own summary counts when present; the walk is
    // still the source of the failing check ids.
    if let Some(counts) = json.get("summary").and_then(|s| s.as_object()) {
        let count = |key: &str| {
            counts
                .get(key)
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
        };
        if let Some(f) = count("FAIL") {
            summary.fails = f;
        }
        if let Some(w) = count("WARN") {
            summary.warns = w;
        }
        if let Some(e) = count("ERROR") {
            summary.errors = e;
        }
    }
    Ok(summary)
}

/// Walk the report and tally worst-status per check. The JSON shape is
/// `{"results": {file: {section: [{"check_id", "worst_status", ...}]}}}`;
/// walk generically so reporter format drift doesn't break the gate.
fn summarize(json: &serde_json::Value) -> QaSummary {
    let mut fails = 0usize;
    let mut warns = 0usize;
    let mut errors = 0usize;
    let mut failed: BTreeMap<String, usize> = BTreeMap::new();

    fn walk(
        v: &serde_json::Value,
        fails: &mut usize,
        warns: &mut usize,
        errors: &mut usize,
        failed: &mut BTreeMap<String, usize>,
    ) {
        match v {
            serde_json::Value::Object(map) => {
                let status = map
                    .get("worst_status")
                    .or_else(|| map.get("result"))
                    .and_then(|s| s.as_str());
                if let Some(status) = status {
                    let id = map
                        .get("check_id")
                        .and_then(|s| s.as_str())
                        .unwrap_or("<unknown>");
                    match status {
                        "FAIL" => {
                            *fails += 1;
                            *failed.entry(id.to_string()).or_insert(0) += 1;
                        }
                        "WARN" => *warns += 1,
                        "ERROR" => {
                            *errors += 1;
                            *failed.entry(format!("{id} (ERROR)")).or_insert(0) += 1;
                        }
                        _ => {}
                    }
                    // A check object; no nested checks below it.
                    return;
                }
                for value in map.values() {
                    walk(value, fails, warns, errors, failed);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item, fails, warns, errors, failed);
                }
            }
            _ => {}
        }
    }
    walk(json, &mut fails, &mut warns, &mut errors, &mut failed);

    let mut failed_checks: Vec<(String, usize)> = failed.into_iter().collect();
    failed_checks.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    QaSummary {
        fails,
        warns,
        errors,
        failed_checks,
    }
}

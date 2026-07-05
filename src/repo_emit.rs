//! Emit a Google Fonts upstream repo scaffold in the virtua-grotesk shape:
//! sources/<UFO> + sources/config.yaml, OFL.txt, README.md, Makefile,
//! AUTHORS/CONTRIBUTORS, requirements.txt, fonts/ gitignored.

use anyhow::{Context, Result};
use std::path::Path;

/// The OFL-1.1 license body (everything after the copyright line),
/// standardized GF template, no Reserved Font Name.
const OFL_BODY: &str = include_str!("../assets/OFL_body.txt");

pub struct RepoSpec<'a> {
    pub family: &'a str,
    pub style: &'a str,
    pub copyright: &'a str,
    pub git_url: &'a str,
    pub designer: &'a str,
    pub ufo_file_name: &'a str,
}

/// Write the repo scaffold into `dir` (created if missing). Returns nothing;
/// the UFO itself is written by the pipeline into `dir/sources/` beforehand.
pub fn emit(dir: &Path, spec: &RepoSpec) -> Result<()> {
    let sources = dir.join("sources");
    std::fs::create_dir_all(&sources)
        .with_context(|| format!("Cannot create {sources:?}"))?;

    let write = |path: &Path, contents: String| -> Result<()> {
        std::fs::write(path, contents).with_context(|| format!("Cannot write {path:?}"))
    };

    // sources/config.yaml — gftools-builder config.
    write(
        &sources.join("config.yaml"),
        format!(
            "sources:\n  - {}\nfamilyName: \"{}\"\noutputDir: ../fonts\n\
             buildVariable: false\nbuildOTF: false\nbuildWebfont: false\n\
             autohintTTF: false\n",
            spec.ufo_file_name, spec.family
        ),
    )?;

    // OFL.txt — first line is the copyright string, byte-identical to
    // name ID 0.
    write(
        &dir.join("OFL.txt"),
        format!("{}\n\n{}", spec.copyright, OFL_BODY),
    )?;

    // README.md stub.
    write(
        &dir.join("README.md"),
        format!(
            "# {family}\n\n\
             {family} is a revival traced from a printed type specimen by\n\
             [img2ufo](https://github.com/eliheuer/img2ufo). This repository\n\
             follows the Google Fonts upstream conventions.\n\n\
             ## Building\n\n\
             ```sh\n\
             make setup   # create .venv and install the toolchain\n\
             make build   # build TTFs into fonts/\n\
             make qa      # fontspector googlefonts profile\n\
             ```\n\n\
             ## License\n\n\
             This Font Software is licensed under the SIL Open Font License,\n\
             Version 1.1. See [OFL.txt](OFL.txt).\n",
            family = spec.family
        ),
    )?;

    // Makefile — adapted from virtua-grotesk (build/qa/clean subset).
    write(
        &dir.join("Makefile"),
        format!(
            "PYTHON ?= ./.venv/bin/python\n\n\
             .PHONY: help setup build qa clean\n\n\
             help:\n\
             \t@printf '%s\\n' \\\n\
             \t\t'{family} workflow:' \\\n\
             \t\t'  make setup   Create .venv and install requirements' \\\n\
             \t\t'  make build   Build TTFs into fonts/' \\\n\
             \t\t'  make qa      Run fontspector Google Fonts profile' \\\n\
             \t\t'  make clean   Remove generated build outputs'\n\n\
             setup:\n\
             \tpython3 -m venv .venv\n\
             \t$(PYTHON) -m pip install -r requirements.txt\n\n\
             build:\n\
             \trm -rf fonts build build.ninja .ninja_log sources/build.ninja sources/.ninja_log\n\
             \t. .venv/bin/activate && gftools builder sources/config.yaml\n\n\
             qa: \n\
             \tfontspector -p googlefonts fonts/ttf/*.ttf\n\n\
             clean:\n\
             \trm -rf fonts build build.ninja .ninja_log sources/build.ninja sources/.ninja_log\n",
            family = spec.family
        ),
    )?;

    // .gitignore — built artifacts stay out of version control.
    write(
        &dir.join(".gitignore"),
        "# Built fonts - keep out of version control\n\
         fonts/\n\
         build/\n\
         build.ninja\n\
         .ninja_log\n\
         sources/build.ninja\n\
         sources/.ninja_log\n\
         instance_ufo/\n\
         sources/instance_ufos/\n\
         master_ufo/\n\
         .venv/\n\
         __pycache__/\n\
         *.py[cod]\n\
         .DS_Store\n"
            .to_string(),
    )?;

    write(&dir.join("AUTHORS.txt"), format!("{}\n", spec.designer))?;
    write(
        &dir.join("CONTRIBUTORS.txt"),
        format!("{}\nimg2ufo pipeline\n", spec.designer),
    )?;
    write(
        &dir.join("requirements.txt"),
        "fontmake\nfonttools\ngftools[qa]\n".to_string(),
    )?;

    // sources/README.md — what lives here.
    write(
        &sources.join("README.md"),
        format!(
            "# Sources\n\n\
             - `{ufo}` — the {family} {style} master, traced by img2ufo.\n\
             - `config.yaml` — gftools-builder configuration.\n\n\
             Upstream repository: {url}\n",
            ufo = spec.ufo_file_name,
            family = spec.family,
            style = spec.style,
            url = spec.git_url
        ),
    )?;

    Ok(())
}

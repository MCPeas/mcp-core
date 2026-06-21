// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Build the embedded web-UI shell when the `web-ui` feature is enabled.
//!
//! Vendors the browser dependencies (declared in `web/package.json`), compiles the shell's
//! TypeScript/SCSS from `web/src`, renders `index.html` with the import map injected, and
//! writes the static dist to `$OUT_DIR/web-ui-dist`. The dist path is exported to the library
//! as `MCP_UI_DIST` (read in `src/web/shell.rs`), where the `web` module's `ServeDir` serves
//! it. Only `web/src` is baked, so `web/package.json` and the dev tooling beside it are never
//! served. Does nothing unless the `web-ui` feature is enabled, so non-UI consumers pay no
//! vendoring / network / transform cost (the build-dependency still compiles, an accepted cost).

use std::path::PathBuf;

fn main() {
    // The public landing/login page (the `web` feature) inlines a compiled stylesheet; build it
    // whenever `web` is on -- independent of the heavier `web-ui` shell bake below.
    if std::env::var_os("CARGO_FEATURE_WEB").is_some() {
        if let Err(err) = build_landing_css() {
            panic!("mcp-core: failed to compile the landing stylesheet: {err}");
        }
    }
    // `[build-dependencies]` always compile; the shell bake only runs for `web-ui` builds.
    if std::env::var_os("CARGO_FEATURE_WEB_UI").is_none() {
        return;
    }
    if let Err(err) = build_shell() {
        panic!("mcp-core: failed to build the web-ui shell: {err}");
    }
}

/// Compile the standalone landing/login stylesheet (`src/web/landing.scss`) to `$OUT_DIR/landing.css`
/// and export its path as `MCP_LANDING_CSS`; `src/web/landing.rs` inlines it into the `/` page.
fn build_landing_css() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let scss = manifest_dir.join("src/web/landing.scss");
    let out = PathBuf::from(std::env::var("OUT_DIR")?).join("landing.css");
    // web_modules owns SCSS compilation (it wraps grass + applies our compressed-output default).
    let css = web_modules::scss::compile_file(&scss, &[]).map_err(|e| e.to_string())?;
    std::fs::write(&out, css)?;
    println!("cargo:rustc-env=MCP_LANDING_CSS={}", out.display());
    println!("cargo:rerun-if-changed=src/web/landing.scss");
    Ok(())
}

fn build_shell() -> web_modules::Result<()> {
    use web_modules::templates::{render_file, Context};
    use web_modules::typescript::TranspileOptions;
    use web_modules::vendor::{self, PackageSpec};

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let web = manifest_dir.join("web");
    let src = web.join("src");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("web-ui-dist");
    std::fs::create_dir_all(&out)?;

    // Markdown rendering (the `web-markdown` feature) vendors `marked` and tells the shell to use
    // it; without it, `marked` is left out of the bundle entirely.
    let markdown = std::env::var_os("CARGO_FEATURE_WEB_MARKDOWN").is_some();

    // Browser dependencies + their versions come from web/package.json's `dependencies` -- the
    // single source of truth, shared with the tsc/eslint tooling; nothing is pinned here.
    // web_modules vendors each (no transitive npm resolution). bootstrap is consumed only via
    // its SCSS, so it gets no import-map entry / JS (the one non-version tweak we apply).
    let specs: Vec<PackageSpec> = vendor::specs_from_package_json(&web.join("package.json"))?
        .into_iter()
        // `marked` ships only when `web-markdown` is on (the shell imports it lazily).
        .filter(|spec| markdown || spec.name() != "marked")
        .map(|spec| {
            if spec.name() == "bootstrap" {
                spec.no_imports()
            } else {
                spec
            }
        })
        .collect();
    // The shell is served under /ui/ (the landing/info page owns /), so its assets and the
    // vendored modules resolve under /ui/ too.
    let mut importmap = vendor::vendor(&out.join("web_modules"), "/ui/web_modules", &specs)?;
    // Our own emitted public API module - consumers import { ... } from "mcp-ui".
    importmap.insert("mcp-ui", "/ui/api/mcp-ui.js");

    // Only web/src is compiled + served; web/package.json and the tooling beside it stay out.
    web_modules::typescript::compile_directory_with(&src, &out, &TranspileOptions::default())?;
    web_modules::scss::compile_directory(&src, &out, &[out.as_path()])?;
    web_modules::static_files::copy_static(&src, &out)?;

    // Vendor the oxc legacy-decorator runtime the transform emits
    // (@oxc-project/runtime/helpers/decorate) so the @property/@state shell resolves it.
    importmap.extend(web_modules::build::vendor_transform_runtime(
        &out,
        "/ui/web_modules",
    )?);

    // CSP: hash the inline import-map script's content - the bytes between the tags, which is
    // what the browser hashes - so the runtime CSP can allow exactly it via script-src
    // 'sha256-...' while still forbidding any other inline script. Hash the same string we embed.
    let script_tag = importmap.to_script_tag();
    let inner = script_tag
        .split_once('>')
        .and_then(|(_, rest)| rest.rsplit_once("</script>"))
        .map(|(content, _)| content)
        .unwrap_or("");
    use base64::Engine as _;
    use sha2::Digest as _;
    let hash =
        base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(inner.as_bytes()));
    println!("cargo:rustc-env=MCP_UI_CSP_SCRIPT_HASH=sha256-{hash}");

    let mut ctx = Context::new();
    ctx.insert("importmap", &script_tag);
    // The shell reads this (a meta tag) to decide whether to render markdown via `marked`.
    ctx.insert("markdown", &markdown);
    let index_html = render_file(&src.join("index.html.tera"), &ctx)?;
    std::fs::write(out.join("index.html"), index_html)?;

    // Export the dist path to the library (read via env! in src/web/shell.rs).
    println!("cargo:rustc-env=MCP_UI_DIST={}", out.display());

    // Re-vendor when the dependency manifest changes.
    println!("cargo:rerun-if-changed=web/package.json");

    // Rebuild when any shell source changes - emit every file (a bare directory rerun only
    // notices add/remove, not edits to existing files).
    for entry in walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        println!("cargo:rerun-if-changed={}", entry.path().display());
    }
    Ok(())
}

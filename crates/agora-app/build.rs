//! Compiles the Blueprint files and the GResource bundle, and (for runs from
//! the source tree) the GSettings schema.
//!
//! Meson builds call Cargo with these variables set; plain `cargo build` falls
//! back to development defaults:
//! - `AGORA_APP_ID` (default `io.github.ClickCalickClick.Agora.Devel`)
//! - `AGORA_PROFILE` (`development` or `default`)
//! - `AGORA_LOCALEDIR` (default `/usr/share/locale`)
//! - `AGORA_PKGDATADIR` (unset: the schema compiled here is used)

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../..").canonicalize().unwrap();
    let data = root.join("data");
    let resources = data.join("resources");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    for var in ["AGORA_APP_ID", "AGORA_PROFILE", "AGORA_LOCALEDIR", "AGORA_PKGDATADIR"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    println!("cargo:rerun-if-changed={}", resources.display());
    println!(
        "cargo:rerun-if-changed={}",
        data.join("io.github.ClickCalickClick.Agora.gschema.xml.in").display()
    );

    // Blueprint → .ui, keeping the directory layout under ui/.
    let blueprints = files_with_ext(&resources.join("ui"), "blp");
    let ui_out = out.join("ui");
    fs::create_dir_all(&ui_out).unwrap();
    if !blueprints.is_empty() {
        let status = Command::new("blueprint-compiler")
            .arg("batch-compile")
            .arg(&ui_out)
            .arg(resources.join("ui"))
            .args(&blueprints)
            .status()
            .expect("blueprint-compiler is required to build Agora");
        assert!(status.success(), "blueprint-compiler failed");
    }

    // GResource XML listing every compiled .ui and every stylesheet.
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gresources>\n  <gresource prefix=\"/io/github/ClickCalickClick/Agora\">\n",
    );
    for ui in files_with_ext(&ui_out, "ui") {
        let rel = ui.strip_prefix(&out).unwrap().display().to_string();
        xml += &format!(
            "    <file compressed=\"true\" preprocess=\"xml-stripblanks\" alias=\"{rel}\">{}</file>\n",
            ui.display()
        );
    }
    for css in files_with_ext(&resources.join("style"), "css") {
        let rel = css.strip_prefix(&resources).unwrap().display().to_string();
        xml += &format!(
            "    <file compressed=\"true\" alias=\"{rel}\">{}</file>\n",
            css.display()
        );
    }
    for icon in files_with_ext(&resources.join("icons"), "svg") {
        let rel = icon.strip_prefix(&resources).unwrap().display().to_string();
        xml += &format!(
            "    <file compressed=\"true\" preprocess=\"xml-stripblanks\" alias=\"{rel}\">{}</file>\n",
            icon.display()
        );
    }
    xml += "  </gresource>\n</gresources>\n";
    let xml_path = out.join("agora.gresource.xml");
    fs::write(&xml_path, xml).unwrap();
    glib_build_tools::compile_resources(&[out.as_path()], xml_path.to_str().unwrap(), "agora.gresource");

    // Development schema, used when AGORA_PKGDATADIR is unset.
    let schema_in = data.join("io.github.ClickCalickClick.Agora.gschema.xml.in");
    let schema_dir = out.join("schemas");
    fs::create_dir_all(&schema_dir).unwrap();
    if schema_in.exists() {
        fs::copy(
            &schema_in,
            schema_dir.join("io.github.ClickCalickClick.Agora.gschema.xml"),
        )
        .unwrap();
        let status = Command::new("glib-compile-schemas")
            .arg("--strict")
            .arg(&schema_dir)
            .status()
            .expect("glib-compile-schemas is required");
        assert!(status.success(), "glib-compile-schemas failed");
    }

    let app_id = env::var("AGORA_APP_ID").unwrap_or_else(|_| "io.github.ClickCalickClick.Agora.Devel".into());
    let profile = env::var("AGORA_PROFILE").unwrap_or_else(|_| "development".into());
    let localedir = env::var("AGORA_LOCALEDIR").unwrap_or_else(|_| "/usr/share/locale".into());
    println!("cargo:rustc-env=AGORA_APP_ID={app_id}");
    println!("cargo:rustc-env=AGORA_PROFILE={profile}");
    println!("cargo:rustc-env=AGORA_LOCALEDIR={localedir}");
    println!("cargo:rustc-env=AGORA_DEV_SCHEMA_DIR={}", schema_dir.display());
    match env::var("AGORA_PKGDATADIR") {
        Ok(dir) => println!("cargo:rustc-env=AGORA_PKGDATADIR={dir}"),
        Err(_) => println!("cargo:rustc-env=AGORA_PKGDATADIR="),
    }
}

fn files_with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_with_ext(&path, ext));
        } else if path.extension().is_some_and(|e| e == ext) {
            found.push(path);
        }
    }
    found.sort();
    found
}

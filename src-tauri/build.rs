use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        tauri_build::build()
    }
    generate_embedded_web();
}

fn generate_embedded_web() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let web_root = manifest_dir
        .parent()
        .map(|path| path.join("build"))
        .unwrap_or_else(|| manifest_dir.join("../build"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let generated = out_dir.join("embedded_web.rs");

    println!("cargo:rerun-if-changed={}", web_root.display());

    let mut files = Vec::new();
    collect_files(&web_root, &web_root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut source = String::new();
    source.push_str(&format!(
        "pub const EMBEDDED_WEB_ASSET_COUNT: usize = {};\n",
        files.len()
    ));
    source.push_str("pub fn embedded_web_asset(path: &str) -> Option<&'static [u8]> {\n");
    source.push_str("    match path {\n");
    for (relative, absolute) in &files {
        source.push_str(&format!(
            "        {:?} => Some(include_bytes!({:?})),\n",
            relative,
            absolute.to_string_lossy()
        ));
    }
    source.push_str("        _ => None,\n");
    source.push_str("    }\n");
    source.push_str("}\n");

    fs::write(generated, source).expect("write embedded_web.rs");
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else if path.is_file() {
            if let Ok(relative) = path.strip_prefix(root) {
                files.push((relative.to_string_lossy().replace('\\', "/"), path));
            }
        }
    }
}

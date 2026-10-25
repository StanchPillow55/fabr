use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(fabr_ui)");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let app = manifest.join("../../app");
    let dist = app.join("dist");
    if dist.is_dir() {
        println!("cargo::rerun-if-changed={}", dist.display());
    } else if app.is_dir() {
        println!("cargo::rerun-if-changed={}", app.display());
    }
    if dist.join("index.html").is_file() {
        let dist = dist.canonicalize().expect("canonical dist");
        println!("cargo::rustc-cfg=fabr_ui");
        println!("cargo::rustc-env=FABR_UI_DIR={}", dist.display());
    }
}

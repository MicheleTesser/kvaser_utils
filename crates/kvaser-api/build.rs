use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let sdk_include = workspace_root.join("third_party/kvaser/kvlibsdk/include");
    let linuxcan_include = workspace_root.join("third_party/kvaser/linuxcan/include");

    println!("cargo:rerun-if-changed={}", sdk_include.display());
    println!("cargo:rerun-if-changed={}", linuxcan_include.display());

    let bindings = bindgen::Builder::default()
        .header(sdk_include.join("kvmlib.h").to_string_lossy())
        .header(sdk_include.join("kvlclib.h").to_string_lossy())
        .header(sdk_include.join("kvaDbLib.h").to_string_lossy())
        .clang_arg(format!("-I{}", sdk_include.display()))
        .clang_arg(format!("-I{}", linuxcan_include.display()))
        .allowlist_type("kvmLogEventEx")
        .allowlist_type("KvaDbHnd")
        .allowlist_var("kvmLOG_TYPE_.*")
        .allowlist_var("kvlcERR_.*")
        .allowlist_var("kvlcOK")
        .layout_tests(false)
        .generate_comments(false)
        .generate()
        .expect("generate Kvaser bindings");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    bindings
        .write_to_file(out_dir.join("kvaser_bindings.rs"))
        .expect("write Kvaser bindings");
}

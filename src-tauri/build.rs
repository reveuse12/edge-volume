fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "macos" {
        cc::Build::new().file("native/mac.c").compile("edge_native");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
    }
    println!("cargo:rerun-if-changed=native/mac.c");
    tauri_build::build()
}

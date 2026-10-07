fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "macos" {
        cc::Build::new().file("native/mac.c").compile("edge_native");
        cc::Build::new()
            .file("native/overlay.m")
            .compile("edge_overlay");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        cc::Build::new()
            .cpp(true)
            .file("native/windows.cpp")
            .flag_if_supported("/std:c++17")
            .compile("edge_windows");
        println!("cargo:rustc-link-lib=ole32");
        println!("cargo:rustc-link-lib=user32");
    }
    println!("cargo:rerun-if-changed=native/mac.c");
    println!("cargo:rerun-if-changed=native/windows.cpp");
    println!("cargo:rerun-if-changed=native/overlay.m");
    tauri_build::build()
}

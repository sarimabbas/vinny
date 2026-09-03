use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let target = std::env::var("TARGET").expect("TARGET");
    let swift_target = if target.starts_with("aarch64-") {
        "arm64-apple-macosx12.3"
    } else if target.starts_with("x86_64-") {
        "x86_64-apple-macosx12.3"
    } else {
        panic!("Vinny supports only macOS targets");
    };
    let swift_source = format!("{manifest}/src/VinnyUI.swift");
    let swift_object = out.join("VinnyUI.o");

    println!("cargo:rerun-if-changed=Info.plist");
    println!("cargo:rerun-if-changed=src/VinnyUI.swift");

    let status = Command::new("xcrun")
        .args([
            "swiftc",
            "-parse-as-library",
            "-target",
            swift_target,
            "-O",
            "-c",
            &swift_source,
            "-o",
        ])
        .arg(&swift_object)
        .status()
        .expect("run swiftc");
    assert!(status.success(), "compile SwiftUI");

    println!("cargo:rustc-link-arg={}", swift_object.display());
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=CoreText");
    println!("cargo:rustc-link-lib=framework=SwiftUI");
    println!("cargo:rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,{manifest}/Info.plist");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");

    if let Ok(output) = Command::new("xcode-select").arg("-p").output() {
        let developer = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !developer.is_empty() {
            println!(
                "cargo:rustc-link-arg=-Wl,-rpath,{developer}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx"
            );
            println!(
                "cargo:rustc-link-arg=-Wl,-rpath,{developer}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx"
            );
        }
    }
}

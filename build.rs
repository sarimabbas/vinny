use std::process::Command;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    println!("cargo:rerun-if-changed=Info.plist");
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

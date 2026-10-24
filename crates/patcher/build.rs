use std::{env, fs, path::PathBuf, process::Command};

/// The patcher no longer carries any patch data: language packs are published
/// as release assets and downloaded on first use (see `patch-build pack`). The
/// build script only compiles the Slint UI and attaches the window icon, so a
/// patcher.exe build needs no payload directory at all.
fn main() {
    slint_build::compile("ui/patcher.slint").unwrap();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=ui/patcher.slint");
    // The release tag decides where language packs are downloaded from, so a
    // new tag must rebuild the binary even when no source changed.
    println!("cargo:rerun-if-env-changed=DORAEMON_PATCHER_RELEASE_TAG");

    // Windows icon resource
    if env::var("TARGET").is_ok_and(|target| target.ends_with("windows-gnu")) {
        let rc = out_dir.join("patcher-icon.rc");
        let icon = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../content/assets/icons/patcher.ico");
        let rc_text = format!("1 ICON \"{}\"\n", icon.display().to_string().replace('\\', "/"));
        fs::write(&rc, rc_text).expect("write icon resource");
        let object = out_dir.join("patcher-icon.o");
        let status = Command::new("x86_64-w64-mingw32-windres")
            .args(["-i"])
            .arg(&rc)
            .args(["-o"])
            .arg(&object)
            .args(["-O", "coff"])
            .status()
            .expect("run windres");
        assert!(status.success(), "compile patcher icon resource");
        println!("cargo:rustc-link-arg={}", object.display());
    }
}

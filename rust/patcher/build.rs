use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (variable, name) in [
        ("DORAEMON_PATCH_PAYLOAD_ENGLISH", "english-payload.bin"),
        ("DORAEMON_PATCH_PAYLOAD_VIETNAMESE", "vietnamese-payload.bin"),
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
        let output = out_dir.join(name);
        if let Some(source) = env::var_os(variable) {
            println!("cargo:rerun-if-changed={}", PathBuf::from(&source).display());
            fs::copy(source, output).expect("copy patch payload");
        } else {
            fs::write(output, []).expect("write empty development payload");
        }
    }
    // GNU Windows builds use the patcher artwork as the executable icon. The
    // game icons are applied later by the patcher itself, after a language is
    // chosen.
    if env::var("TARGET").is_ok_and(|target| target.ends_with("windows-gnu")) {
        let rc = out_dir.join("patcher-icon.rc");
        fs::write(&rc, "1 ICON \"assets/icons/patcher.ico\"\n").expect("write icon resource");
        let icon = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../assets/icons/patcher.ico");
        let rc_text = format!("1 ICON \"{}\"\n", icon.display().to_string().replace('\\', "/"));
        fs::write(&rc, rc_text).expect("write icon resource");
        let object = out_dir.join("patcher-icon.o");
        let status = Command::new("x86_64-w64-mingw32-windres")
            .args(["-i"]).arg(&rc).args(["-o"]).arg(&object).args(["-O", "coff"])
            .status().expect("run windres");
        assert!(status.success(), "compile patcher icon resource");
        println!("cargo:rustc-link-arg={}", object.display());
    }
}

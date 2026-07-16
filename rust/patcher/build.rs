use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-env-changed=DORAEMON_PATCH_PAYLOAD");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("payload.bin");
    if let Some(source) = env::var_os("DORAEMON_PATCH_PAYLOAD") {
        println!(
            "cargo:rerun-if-changed={}",
            PathBuf::from(&source).display()
        );
        fs::copy(source, &output).expect("copy patch payload");
    } else {
        fs::write(output, []).expect("write empty development payload");
    }
}

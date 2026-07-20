use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Multipart format (new): read a directory of .dmpatch part files
    let part_targets = [
        "loc-doraemon.dmpatch",
        "loc-nobita.dmpatch",
        "loc-dorami.dmpatch",
        "loc-shizuka.dmpatch",
        "loc-suneo.dmpatch",
        "loc-gian.dmpatch",
        "loc-others.dmpatch",
        "sprites.dmpatch",
        "runtime.dmpatch",
    ];
    for (variable, _name) in [
        ("DORAEMON_PATCH_PARTS_ENGLISH", "english-parts.bin"),
        (
            "DORAEMON_PATCH_PARTS_VIETNAMESE",
            "vietnamese-parts.bin",
        ),
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
        if let Some(source) = env::var_os(variable) {
            let dir = PathBuf::from(&source);
            for target in &part_targets {
                let part_path = dir.join(target);
                if part_path.exists() {
                    println!("cargo:rerun-if-changed={}", part_path.display());
                }
            }
        }
        // Always re-run to pick up changes inside the directory.
        // The rerun-if-env-changed above plus the rerun-if-changed for each
        // part file ensure the blob is regenerated when any part changes.
    }

    // Now actually write the blobs
    for (variable, name) in [
        ("DORAEMON_PATCH_PARTS_ENGLISH", "english-parts.bin"),
        (
            "DORAEMON_PATCH_PARTS_VIETNAMESE",
            "vietnamese-parts.bin",
        ),
    ] {
        let output = out_dir.join(name);
        if let Some(source) = env::var_os(variable) {
            let dir = PathBuf::from(&source);
            let mut parts: Vec<Vec<u8>> = Vec::new();
            let mut found = 0u16;
            for target in &part_targets {
                let part_path = dir.join(target);
                if part_path.exists() {
                    let bytes = fs::read(&part_path).expect("read part file");
                    found += 1;
                    println!("cargo:warning=Embedding {} {name}: {target} ({size} bytes)", variable, size = bytes.len());
                    parts.push(bytes);
                } else {
                    println!("cargo:warning={variable} {name}: MISSING {target} — inserting empty placeholder");
                    parts.push(Vec::new());
                }
            }
            let mut blob = Vec::new();
            blob.extend_from_slice(b"DPART");
            blob.extend_from_slice(&(parts.len() as u16).to_le_bytes());
            // The blob header is five magic bytes plus a two-byte part count.
            // Offsets must begin after all seven header bytes and the table.
            let mut data_start = 7 + parts.len() * 8;
            for bytes in &parts {
                blob.extend_from_slice(&(data_start as u32).to_le_bytes());
                blob.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                data_start += bytes.len();
            }
            for bytes in &parts {
                blob.extend_from_slice(bytes);
            }
            fs::write(&output, &blob).expect("write parts blob");
            println!("cargo:warning=Wrote {name}: {found}/{count} parts, {blob_size} bytes blob", count = part_targets.len(), blob_size = blob.len());
            eprintln!("DIAG: build.rs {name}: dir={dir:?} parts={found}/{count} blob={blob_size}B", count = part_targets.len(), blob_size = blob.len());
        } else {
            fs::write(output, []).expect("write empty parts blob");
            eprintln!("DIAG: build.rs {name}: env var NOT SET, wrote empty blob");
        }
    }

    // Monolithic format (legacy): single .dmpatch file
    for (variable, name) in [
        ("DORAEMON_PATCH_PAYLOAD_ENGLISH", "english-payload.bin"),
        (
            "DORAEMON_PATCH_PAYLOAD_VIETNAMESE",
            "vietnamese-payload.bin",
        ),
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
        let output = out_dir.join(name);
        if let Some(source) = env::var_os(variable) {
            println!(
                "cargo:rerun-if-changed={}",
                PathBuf::from(&source).display()
            );
            fs::copy(source, output).expect("copy patch payload");
        } else {
            fs::write(output, []).expect("write empty development payload");
        }
    }

    // Windows icon resource
    if env::var("TARGET").is_ok_and(|target| target.ends_with("windows-gnu")) {
        let rc = out_dir.join("patcher-icon.rc");
        let icon = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../assets/icons/patcher.ico");
        let rc_text = format!(
            "1 ICON \"{}\"\n",
            icon.display().to_string().replace('\\', "/")
        );
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

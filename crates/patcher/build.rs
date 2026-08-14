use std::{env, fs, path::PathBuf, process::Command};

/// Header prefix for zstd-compressed embedded data. Four magic bytes followed
/// by the little-endian decompressed length, then a zstd frame.
const DZC_MAGIC: &[u8; 4] = b"DZC1";
const ZSTD_LEVEL: i32 = 19;

fn dzc_compress(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() / 2 + 16);
    output.extend_from_slice(DZC_MAGIC);
    output.extend_from_slice(&(data.len() as u64).to_le_bytes());
    output.extend_from_slice(
        &zstd::bulk::compress(data, ZSTD_LEVEL).expect("zstd compress patch data"),
    );
    output
}

/// Build a DPART blob from a directory of .dmpatch part files.
fn build_parts_blob(dir: &PathBuf, label: &str) -> Vec<u8> {
    let component_targets = ["dubbing.dmpatch", "sprites.dmpatch", "runtime.dmpatch"];
    let legacy_targets = [
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
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let mut found = 0u16;
    let targets = if dir.join(component_targets[0]).exists() {
        &component_targets[..]
    } else {
        &legacy_targets[..]
    };
    for target in targets {
        let part_path = dir.join(target);
        if part_path.exists() {
            let bytes = fs::read(&part_path).expect("read part file");
            found += 1;
            println!(
                "cargo:warning=Embedding {label} part: {target} ({size} bytes)",
                size = bytes.len()
            );
            parts.push(bytes);
        } else {
            println!(
                "cargo:warning={label}: MISSING {target} - inserting empty placeholder"
            );
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
    println!(
        "cargo:warning={label} parts: {found}/{count} components, {blob_size} bytes blob",
        count = targets.len(),
        blob_size = blob.len()
    );
    blob
}

fn main() {
    slint_build::compile("ui/patcher.slint").unwrap();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Multipart format (new): read a directory of .dmpatch part files
    let component_targets = ["dubbing.dmpatch", "sprites.dmpatch", "runtime.dmpatch"];
    for (variable, _label) in [
        ("DORAEMON_PATCH_PARTS_ENGLISH", "English"),
        ("DORAEMON_PATCH_PARTS_VIETNAMESE", "Vietnamese"),
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
        if let Some(source) = env::var_os(variable) {
            let dir = PathBuf::from(&source);
            let targets = if dir.join(component_targets[0]).exists() {
                &component_targets[..]
            } else {
                &[
                    "loc-doraemon.dmpatch",
                    "loc-nobita.dmpatch",
                    "loc-dorami.dmpatch",
                    "loc-shizuka.dmpatch",
                    "loc-suneo.dmpatch",
                    "loc-gian.dmpatch",
                    "loc-others.dmpatch",
                    "sprites.dmpatch",
                    "runtime.dmpatch",
                ][..]
            };
            for target in targets {
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

    // Build both languages' DPART blobs (empty when a directory is not set),
    // then embed them together in a single zstd-compressed bundle. Compressing
    // the two languages as one stream lets zstd reuse the near-identical
    // runtime component (the cnc-ddraw wrapper) shared by both.
    let mut bundle = Vec::new();
    for (variable, label) in [
        ("DORAEMON_PATCH_PARTS_ENGLISH", "English"),
        ("DORAEMON_PATCH_PARTS_VIETNAMESE", "Vietnamese"),
    ] {
        let blob = match env::var_os(variable) {
            Some(source) => {
                let dir = PathBuf::from(&source);
                let blob = build_parts_blob(&dir, label);
                eprintln!("DIAG: build.rs {label} parts blob={}B dir={dir:?}", blob.len());
                blob
            }
            None => {
                eprintln!("DIAG: build.rs {label} parts: env var NOT SET, using empty blob");
                Vec::new()
            }
        };
        bundle.extend_from_slice(&(blob.len() as u64).to_le_bytes());
        bundle.extend_from_slice(&blob);
    }
    let compressed = dzc_compress(&bundle);
    fs::write(out_dir.join("parts-bundle.bin"), &compressed).expect("write parts bundle");
    println!(
        "cargo:warning=Parts bundle: {} bytes zstd-19 (raw {bundle_size} bytes)",
        compressed.len(),
        bundle_size = bundle.len()
    );
    eprintln!(
        "DIAG: build.rs parts bundle: raw={}B compressed={}B",
        bundle.len(),
        compressed.len()
    );

    // Monolithic format (legacy): single .dmpatch file, zstd-compressed the
    // same way. Used by package/portable builds that have no multipart data.
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
            let bytes = fs::read(&source).expect("read patch payload");
            let compressed = dzc_compress(&bytes);
            fs::write(&output, &compressed).expect("write compressed patch payload");
            eprintln!(
                "DIAG: build.rs {name}: raw={}B compressed={}B",
                bytes.len(),
                compressed.len()
            );
        } else {
            fs::write(output, []).expect("write empty development payload");
            eprintln!("DIAG: build.rs {name}: env var NOT SET, wrote empty blob");
        }
    }

    // Windows icon resource
    if env::var("TARGET").is_ok_and(|target| target.ends_with("windows-gnu")) {
        let rc = out_dir.join("patcher-icon.rc");
        let icon = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../content/assets/icons/patcher.ico");
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

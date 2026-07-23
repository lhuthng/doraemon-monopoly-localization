use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use doraemon_game_patch::{
    cue, hash,
    payload::{
        self, BundledFile, FilePatch, Language, PatchProfile, Payload, PayloadPart, RequiredFile,
        TargetName,
    },
    pe, strings, sysfont, voice,
};

const FILES: &[&str] = &[
    "Doraemon.exe",
    "strings.dat",
    "sysfont.dat",
    "Sprite1.dat",
    "sprite2.dat",
    "bitmaps.dat",
    "voice.dat",
];

const RESOURCE_FILES: &[&str] = &[
    "strings.dat",
    "sysfont.dat",
    "Sprite1.dat",
    "sprite2.dat",
    "bitmaps.dat",
    "voice.dat",
];

const SUPPORTED: &[(&str, &[&str])] = &[
    (
        "strings.dat",
        &[
            "9ecce72afcef20e472d70d5d3b642887202ea85fc05e2e22aa05694de972dcec",
            "2cab5b50a88b6ddaba1a2829d555ad5c0d296b9a6d7ad20af7604745341ad38e",
        ],
    ),
    (
        "sysfont.dat",
        &[
            "d4a9885a0358740427d7e5fdfb44d0c9d77a2acab8623549e0a5e4ac5def4510",
            "8fedc4ad400c55ee65c60baa229894e862b2e1a2d9e0a2d58371495200c4e4a0",
        ],
    ),
    (
        "Sprite1.dat",
        &[
            "3272fa334ad168126b8ea07983f74a2f0e51196c8bf71f181128d9e6f13eb7c4",
            "af68d62e6aefe34ab4bf466898ee532c2647a2569230f7871e059b96346b06b3",
        ],
    ),
    (
        "sprite2.dat",
        &[
            "ec2a6bd53fbced5fc1be29d95e8c1e57729c0b5c4ed3fd4e9fd11694e4123ecc",
            "5789cc11b7b005009dcd645bf11732178544633f6b9c9c40361640a92f9ecf1e",
        ],
    ),
    (
        "bitmaps.dat",
        &["9bba998ed68836a2db00316a2df51901533032025a7178a1f5ea560c8d5b63c0"],
    ),
    (
        "voice.dat",
        &[
            "e1493bad6c543fc5888f4524c166df55fa8a095c9390d646b60e314fd8c89a85",
            "4cf31414b732d523432c36d410523e5cac4fde2b3e7ad5f367e4d7216a00e9e2",
        ],
    ),
];

fn usage() -> ! {
    eprintln!("Usage:\n  patch-build vi-font --input SYSFONT.DAT --output SYSFONT.DAT\n  patch-build extract-audio --cue DORAEMON.CUE --output DoraemonMusic.wav\n  patch-build release --language english|vietnamese --base-dir DIR --target-dir DIR --output-dir DIR [--target all|doraemon|nobita|dorami|shizuka|suneo|gian|others|sprites|runtime] [--cnc-ddraw-dir DIR] [--target x86_64-pc-windows-gnu] [--payload-only]\n  patch-build release-parts --language english|vietnamese --base-dir DIR --target-dir DIR --output-dir DIR [--target all|doraemon|nobita|dorami|shizuka|suneo|gian|others|sprites|runtime] [--cnc-ddraw-dir DIR]\n  patch-build materialize --payload PATCH.dmpatch --base-dir DIR --output-dir DIR\n  patch-build materialize-parts --parts-dir DIR --base-dir DIR --output-dir DIR\n  patch-build universal --output-dir DIR [--english-payload PATCH.dmpatch] [--vietnamese-payload PATCH.dmpatch] [--cnc-ddraw-dir DIR] [--target x86_64-pc-windows-gnu]\n  patch-build package --payload PATCH.dmpatch --output-dir DIR [--cnc-ddraw-dir DIR] [--target x86_64-pc-windows-gnu]");
    std::process::exit(2)
}

fn materialize(arguments: &[String]) -> Result<(), String> {
    let payload_path = PathBuf::from(value(arguments, "--payload").unwrap_or_else(|| usage()));
    let base = PathBuf::from(value(arguments, "--base-dir").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));
    let payload_bytes =
        fs::read(&payload_path).map_err(|error| format!("{}: {error}", payload_path.display()))?;
    let payload = payload::decode(&payload_bytes).map_err(|error| {
        format!(
            "{} is not a valid Doraemon resource payload: {error}",
            payload_path.display()
        )
    })?;
    if payload.language == Language::Custom {
        return Err("portable compatibility payloads do not contain localizable resources".into());
    }
    let profile = payload
        .profiles
        .iter()
        .find(|profile| {
            profile.required.iter().all(|required| {
                read(&base, &required.name)
                    .map(|bytes| {
                        bytes.len() as u64 == required.len && hash::bytes(&bytes) == required.hash
                    })
                    .unwrap_or(false)
            })
        })
        .ok_or("the supplied folder does not contain a supported original resource set")?;
    let strings_patch = payload
        .strings
        .as_ref()
        .ok_or("this language payload has no strings.dat changes")?;

    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let source_strings = read(&base, "strings.dat")?;
    let rebuilt_strings = strings::apply_patch(&source_strings, strings_patch)?;
    fs::write(output.join("strings.dat"), rebuilt_strings)
        .map_err(|error| format!("write strings.dat: {error}"))?;

    for name in RESOURCE_FILES
        .iter()
        .copied()
        .filter(|name| *name != "strings.dat" && *name != "voice.dat")
    {
        let source = read(&base, name)?;
        let rebuilt = match profile.files.iter().find(|patch| patch.name == name) {
            Some(patch) => patch.apply(&source)?,
            None => source,
        };
        fs::write(output.join(name), rebuilt).map_err(|error| format!("write {name}: {error}"))?;
    }
    let source_voice = read(&base, "voice.dat")?;
    let rebuilt_voice = match &payload.voice {
        Some(patch) => voice::apply_patch(&source_voice, patch)?,
        None => source_voice,
    };
    fs::write(output.join("voice.dat"), rebuilt_voice)
        .map_err(|error| format!("write voice.dat: {error}"))?;
    println!(
        "Materialized {} resource files in {} from {}.",
        RESOURCE_FILES.len(),
        output.display(),
        payload.language.label()
    );
    Ok(())
}

fn materialize_parts(arguments: &[String]) -> Result<(), String> {
    let parts_dir = PathBuf::from(value(arguments, "--parts-dir").unwrap_or_else(|| usage()));
    let base = PathBuf::from(value(arguments, "--base-dir").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));

    let mut parts = Vec::new();
    for target in TargetName::all() {
        let part_path = parts_dir.join(target.filename());
        if !part_path.exists() {
            return Err(format!(
                "missing part file: {}",
                part_path.display()
            ));
        }
        let bytes = fs::read(&part_path)
            .map_err(|e| format!("{}: {e}", part_path.display()))?;
        let part = payload::decode_part(&bytes)
            .map_err(|e| format!("{}: {e}", part_path.display()))?;
        parts.push(part);
    }

    let merged = doraemon_game_patch::merge_parts(&parts)?;
    if merged.language == Language::Custom {
        return Err("portable compatibility payloads do not contain localizable resources".into());
    }

    let profile = merged
        .profiles
        .iter()
        .find(|profile| {
            profile.required.iter().all(|required| {
                read(&base, &required.name)
                    .map(|bytes| {
                        bytes.len() as u64 == required.len && hash::bytes(&bytes) == required.hash
                    })
                    .map_err(|_| ())
                    .unwrap_or(false)
            })
        })
        .ok_or("the supplied folder does not contain a supported original resource set")?;

    fs::create_dir_all(&output).map_err(|error| error.to_string())?;

    if let Some(strings_patch) = &merged.strings {
        let source_strings = read(&base, "strings.dat")?;
        let rebuilt_strings = strings::apply_patch(&source_strings, strings_patch)?;
        fs::write(output.join("strings.dat"), rebuilt_strings)
            .map_err(|error| format!("write strings.dat: {error}"))?;
    }

    for name in RESOURCE_FILES
        .iter()
        .copied()
        .filter(|name| *name != "strings.dat" && *name != "voice.dat")
    {
        let source = read(&base, name)?;
        let rebuilt = match profile.files.iter().find(|patch| patch.name == name) {
            Some(patch) => patch.apply(&source)?,
            None => source,
        };
        fs::write(output.join(name), rebuilt).map_err(|error| format!("write {name}: {error}"))?;
    }

    if let Some(voice_patch) = &merged.voice {
        let source_voice = read(&base, "voice.dat")?;
        let rebuilt_voice = voice::apply_patch(&source_voice, voice_patch)?;
        fs::write(output.join("voice.dat"), rebuilt_voice)
            .map_err(|error| format!("write voice.dat: {error}"))?;
    }

    // Copy map files (read-only, not in payload)
    if let Ok(entries) = fs::read_dir(&base) {
        for entry in entries {
            if let Ok(entry) = entry {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.to_lowercase().starts_with("map") && name.ends_with(".dat") {
                    if let Ok(bytes) = fs::read(entry.path()) {
                        let _ = fs::write(output.join(&name), &bytes);
                    }
                }
            }
        }
    }

    println!(
        "Materialized resources from 9 parts in {} to {}.",
        parts_dir.display(),
        output.display()
    );
    Ok(())
}

fn package(arguments: &[String]) -> Result<(), String> {
    let payload_path = PathBuf::from(value(arguments, "--payload").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));
    let payload_bytes =
        fs::read(&payload_path).map_err(|error| format!("{}: {error}", payload_path.display()))?;
    let mut payload = payload::decode(&payload_bytes).map_err(|error| {
        format!(
            "{} is not a valid Doraemon patch payload: {error}",
            payload_path.display()
        )
    })?;
    if payload.language == Language::Custom {
        return Err("package accepts English or Vietnamese language payloads; use portable for the compatibility patcher".into());
    }
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let wrapper = runtime_files(arguments)?;
    let mut temporary_payload = None;
    let build_payload = if wrapper.is_empty() {
        fs::canonicalize(&payload_path).map_err(|error| error.to_string())?
    } else {
        payload.bundled = wrapper;
        let path = output.join(".embedded-payload.dmpatch");
        fs::write(&path, payload::encode(&payload)?).map_err(|error| error.to_string())?;
        let canonical = fs::canonicalize(&path).map_err(|error| error.to_string())?;
        temporary_payload = Some(path);
        canonical
    };
    let rust_target =
        value(arguments, "--target").unwrap_or_else(|| "x86_64-pc-windows-gnu".into());
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new("cargo")
        .current_dir(&workspace)
        .env("DORAEMON_PATCH_PAYLOAD_ENGLISH", &build_payload)
        .env("DORAEMON_PATCH_PAYLOAD_VIETNAMESE", &build_payload)
        .arg("build")
        .arg("--release")
        .arg("-p")
        .arg("doraemon-patcher")
        .arg("--target")
        .arg(&rust_target)
        .status()
        .map_err(|error| format!("start Cargo: {error}"))?;
    if let Some(path) = temporary_payload {
        fs::remove_file(path).ok();
    }
    if !status.success() {
        return Err(format!("Windows patcher build failed with {status}"));
    }
    let built = workspace
        .join("target")
        .join(&rust_target)
        .join("release")
        .join("doraemon-patcher.exe");
    let destination = output.join(format!("Doraemon-{}-Patcher.exe", payload.language.label()));
    fs::copy(&built, &destination).map_err(|error| format!("{}: {error}", built.display()))?;
    let checksum = hash::file(&destination)?;
    fs::write(
        output.join(format!(
            "Doraemon-{}-Patcher.exe.sha256",
            payload.language.label()
        )),
        format!(
            "{}  {}\n",
            hash::hex(&checksum),
            destination.file_name().unwrap().to_string_lossy()
        ),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "Packaged {} from {}.",
        destination.display(),
        payload_path.display()
    );
    Ok(())
}

fn universal(arguments: &[String]) -> Result<(), String> {
    let english_path = value(arguments, "--english-payload").map(PathBuf::from);
    let vietnamese_path = value(arguments, "--vietnamese-payload").map(PathBuf::from);
    let english_dir = value(arguments, "--english-payload-dir").map(PathBuf::from);
    let vietnamese_dir = value(arguments, "--vietnamese-payload-dir").map(PathBuf::from);
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    if english_path.is_none()
        && vietnamese_path.is_none()
        && english_dir.is_none()
        && vietnamese_dir.is_none()
    {
        return Err("universal needs at least one language payload".into());
    }
    let wrapper = runtime_files(arguments)?;
    let target = value(arguments, "--target").unwrap_or_else(|| "x86_64-pc-windows-gnu".into());
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    // Load from parts directory (multipart) or monolithic payload
    fn load_from_parts_dir(dir: &Path) -> Result<Payload, String> {
        let mut parts = Vec::new();
        let targets = [
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
        for target in &targets {
            let part_path = dir.join(target);
            if part_path.exists() {
                let bytes = fs::read(&part_path).map_err(|e| format!("{}: {e}", part_path.display()))?;
                let part = payload::decode_part(&bytes)
                    .map_err(|e| format!("{}: {e}", part_path.display()))?;
                parts.push(part);
            }
        }
        if parts.is_empty() {
            return Err(format!("no valid parts found in {}", dir.display()));
        }
        let merged = doraemon_game_patch::merge_parts(&parts)
            .map_err(|e| format!("merge parts: {e}"))?;
        Ok(merged)
    }

    let mut english = if let Some(ref dir) = english_dir {
        Some(load_from_parts_dir(dir)?)
    } else {
        english_path
            .as_ref()
            .map(|path| payload::decode(&fs::read(path).map_err(|e| e.to_string())?))
            .transpose()?
    };
    let mut vietnamese = if let Some(ref dir) = vietnamese_dir {
        Some(load_from_parts_dir(dir)?)
    } else {
        vietnamese_path
            .as_ref()
            .map(|path| payload::decode(&fs::read(path).map_err(|e| e.to_string())?))
            .transpose()?
    };

    if english
        .as_ref()
        .is_some_and(|payload| payload.language != Language::English)
        || vietnamese
            .as_ref()
            .is_some_and(|payload| payload.language != Language::Vietnamese)
    {
        return Err("universal payload language does not match its option".into());
    }

    // For multipart, use directory-based env vars
    if english_dir.is_some() || vietnamese_dir.is_some() {
        // Inject the optional cnc-ddraw wrapper into each runtime.dmpatch.
        // These files are not bundled at release-parts time (which runs without
        // --cnc-ddraw-dir for contributor builds), so we patch them in here.
        if !wrapper.is_empty() {
            for dir in [&english_dir, &vietnamese_dir].iter().flat_map(|o| o.as_ref()) {
                let runtime_path = dir.join("runtime.dmpatch");
                if runtime_path.exists() {
                    let bytes = fs::read(&runtime_path)
                        .map_err(|e| format!("{}: {e}", runtime_path.display()))?;
                    let mut part = payload::decode_part(&bytes)
                        .map_err(|e| format!("{}: {e}", runtime_path.display()))?;
                    part.bundled = wrapper.clone();
                    part.is_empty = false;
                    fs::write(&runtime_path, payload::encode_part(&part)?)
                        .map_err(|e| format!("{}: {e}", runtime_path.display()))?;
                    eprintln!(
                        "DIAG: injected {} bundled files into {}",
                        wrapper.len(),
                        runtime_path.display()
                    );
                }
            }
        }

        let en_dir = english_dir.map(|d| {
            let canon = fs::canonicalize(&d).unwrap_or(d);
            canon.to_string_lossy().to_string()
        });
        let vi_dir = vietnamese_dir.map(|d| {
            let canon = fs::canonicalize(&d).unwrap_or(d);
            canon.to_string_lossy().to_string()
        });

        // Also embed a merged legacy payload as a runtime fallback. The GUI
        // prefers multipart data, but the fallback keeps a valid language
        // available if an older build or environment cannot decode DPART.
        let en_temp = output.join(".embedded-english-payload.dmpatch");
        let vi_temp = output.join(".embedded-vietnamese-payload.dmpatch");
        if let Some(payload) = &english {
            fs::write(&en_temp, payload::encode(payload)?).map_err(|e| e.to_string())?;
        }
        if let Some(payload) = &vietnamese {
            fs::write(&vi_temp, payload::encode(payload)?).map_err(|e| e.to_string())?;
        }
        let en_payload = english
            .as_ref()
            .map(|_| fs::canonicalize(&en_temp).map_err(|e| e.to_string()))
            .transpose()?;
        let vi_payload = vietnamese
            .as_ref()
            .map(|_| fs::canonicalize(&vi_temp).map_err(|e| e.to_string()))
            .transpose()?;

        let status = Command::new("cargo")
            .current_dir(&workspace)
            .args([
                "build",
                "--release",
                "-p",
                "doraemon-patcher",
                "--target",
                &target,
            ])
            .env(
                "DORAEMON_PATCH_PARTS_ENGLISH",
                en_dir.as_deref().unwrap_or(""),
            )
            .env(
                "DORAEMON_PATCH_PARTS_VIETNAMESE",
                vi_dir.as_deref().unwrap_or(""),
            )
            .env(
                "DORAEMON_PATCH_PAYLOAD_ENGLISH",
                en_payload
                    .as_ref()
                    .map(|path| path.as_os_str())
                    .unwrap_or_else(|| std::ffi::OsStr::new("")),
            )
            .env(
                "DORAEMON_PATCH_PAYLOAD_VIETNAMESE",
                vi_payload
                    .as_ref()
                    .map(|path| path.as_os_str())
                    .unwrap_or_else(|| std::ffi::OsStr::new("")),
            )
            .status()
            .map_err(|e| format!("start Cargo: {e}"))?;
        fs::remove_file(&en_temp).ok();
        fs::remove_file(&vi_temp).ok();
        if !status.success() {
            return Err(format!("Windows patcher build failed with {status}"));
        }
    } else {
        // Monolithic payload fallback
        if let Some(payload) = &mut english {
            payload.bundled = wrapper.clone();
        }
        if let Some(payload) = &mut vietnamese {
            payload.bundled = wrapper;
        }
        let en_temp = output.join(".english-payload.dmpatch");
        let vi_temp = output.join(".vietnamese-payload.dmpatch");
        fs::write(
            &en_temp,
            english
                .map(|payload| payload::encode(&payload))
                .transpose()?
                .unwrap_or_default(),
        )
        .map_err(|e| e.to_string())?;
        fs::write(
            &vi_temp,
            vietnamese
                .map(|payload| payload::encode(&payload))
                .transpose()?
                .unwrap_or_default(),
        )
        .map_err(|e| e.to_string())?;

        let status = Command::new("cargo")
            .current_dir(&workspace)
            .env(
                "DORAEMON_PATCH_PAYLOAD_ENGLISH",
                fs::canonicalize(&en_temp).map_err(|e| e.to_string())?,
            )
            .env(
                "DORAEMON_PATCH_PAYLOAD_VIETNAMESE",
                fs::canonicalize(&vi_temp).map_err(|e| e.to_string())?,
            )
            .args([
                "build",
                "--release",
                "-p",
                "doraemon-patcher",
                "--target",
                &target,
            ])
            .status()
            .map_err(|e| format!("start Cargo: {e}"))?;
        fs::remove_file(&en_temp).ok();
        fs::remove_file(&vi_temp).ok();
        if !status.success() {
            return Err(format!("Windows patcher build failed with {status}"));
        }
    }

    let built = workspace
        .join("target")
        .join(&target)
        .join("release/doraemon-patcher.exe");
    let destination = output.join("patcher.exe");
    fs::copy(&built, &destination).map_err(|e| format!("{}: {e}", built.display()))?;
    fs::write(
        output.join("patcher.exe.sha256"),
        format!("{}  patcher.exe\n", hash::hex(&hash::file(&destination)?)),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        output.join("README.txt"),
        "Doraemon universal patcher\r\n\r\nCopy patcher.exe into the folder containing Doraemon.exe, then run it there.\r\nChoose <original>, English, or Vietnamese, pick the compatibility options you want, and press Apply.\r\nThe patcher always works on its own folder, creates a backup before writing, and keeps the window open so you can read the log.\r\n",
    )
    .map_err(|e| e.to_string())?;
    println!("Built {}.", destination.display());
    Ok(())
}

fn value(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn read(folder: &Path, name: &str) -> Result<Vec<u8>, String> {
    let path = folder.join(name);
    fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
}

fn collect_bundled(
    root: &Path,
    folder: &Path,
    output: &mut Vec<BundledFile>,
) -> Result<(), String> {
    for entry in fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_bundled(root, &path, output)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            output.push(BundledFile {
                name: relative,
                hash: hash::bytes(&bytes),
                bytes,
            });
        }
    }
    Ok(())
}

fn wrapper_files(arguments: &[String]) -> Result<Vec<BundledFile>, String> {
    let Some(folder) = value(arguments, "--cnc-ddraw-dir") else {
        return Ok(Vec::new());
    };
    let folder = PathBuf::from(folder);
    for required in ["ddraw.dll", "ddraw.ini", "cnc-ddraw config.exe"] {
        if !folder.join(required).is_file() {
            return Err(format!(
                "{} is not a complete cnc-ddraw folder (missing {required})",
                folder.display()
            ));
        }
    }
    let mut output = Vec::new();
    collect_bundled(&folder, &folder, &mut output)?;
    output.sort_by(|a, b| a.name.cmp(&b.name));
    println!("Bundled {} cnc-ddraw files.", output.len());
    Ok(output)
}

fn runtime_files(arguments: &[String]) -> Result<Vec<BundledFile>, String> {
    wrapper_files(arguments)
}

fn build_profile(name: &str, base: &Path, target: &Path) -> Result<PatchProfile, String> {
    let mut required = Vec::new();
    let mut patches = Vec::new();
    for file_name in FILES {
        if matches!(*file_name, "strings.dat" | "Doraemon.exe") {
            continue;
        }
        let original = read(base, file_name)?;
        let localized = read(target, file_name)?;
        let expected = SUPPORTED
            .iter()
            .find(|(candidate, _)| candidate == file_name)
            .unwrap()
            .1;
        let actual = hash::hex(&hash::bytes(&original));
        if !expected.contains(&actual.as_str()) {
            return Err(format!(
                "unsupported base {file_name}: SHA-256 {actual}; expected one of {}",
                expected.join(", ")
            ));
        }
        required.push(RequiredFile {
            name: (*file_name).into(),
            hash: hash::bytes(&original),
            len: original.len() as u64,
        });
        if original != localized && !matches!(*file_name, "Doraemon.exe" | "voice.dat") {
            let patch = FilePatch::create(*file_name, &original, &localized)?;
            println!(
                "{name} / {file_name}: {} -> {} bytes (delta {})",
                original.len(),
                localized.len(),
                patch.delta.len()
            );
            patches.push(patch);
        }
    }
    // Executable patching is structural and happens at install time. Keep empty
    // placeholders only for the shared payload format used by the standalone
    // compatibility patcher.
    let empty_exe = FilePatch::create("Doraemon.exe", &[], &[])?;
    Ok(PatchProfile {
        name: name.into(),
        required,
        files: patches,
        executable_plain: None,
        executable_portable: empty_exe,
    })
}

/// Build fingerprints for all resource files.
fn build_fingerprints(base: &Path) -> Result<Vec<RequiredFile>, String> {
    let mut required = Vec::new();
    for file_name in FILES {
        if matches!(*file_name, "Doraemon.exe") {
            // Executable fingerprinting is structural, not whole-file.
            continue;
        }
        let original = read(base, file_name)?;
        let expected = SUPPORTED
            .iter()
            .find(|(candidate, _)| candidate == file_name)
            .ok_or_else(|| format!("no supported hashes for {file_name}"))?
            .1;
        let actual = hash::hex(&hash::bytes(&original));
        if !expected.contains(&actual.as_str()) {
            return Err(format!(
                "unsupported base {file_name}: SHA-256 {actual}; expected one of {}",
                expected.join(", ")
            ));
        }
        required.push(RequiredFile {
            name: (*file_name).into(),
            hash: hash::bytes(&original),
            len: original.len() as u64,
        });
    }
    Ok(required)
}

fn vi_font(arguments: &[String]) -> Result<(), String> {
    let input = PathBuf::from(value(arguments, "--input").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output").unwrap_or_else(|| usage()));
    let extended = sysfont::extend(&fs::read(&input).map_err(|error| error.to_string())?)?;
    fs::write(&output, &extended).map_err(|error| error.to_string())?;
    println!(
        "Wrote {}: {} bytes, {} glyphs.",
        output.display(),
        extended.len(),
        sysfont::parse(&extended)?.glyphs.len()
    );
    Ok(())
}

fn extract_audio(arguments: &[String]) -> Result<(), String> {
    let cue_path = PathBuf::from(value(arguments, "--cue").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output").unwrap_or_else(|| usage()));
    cue::extract(&cue_path, &output)?;
    println!("Wrote {}: {} bytes.", output.display(), cue::WAV_BYTES);
    Ok(())
}

/// Build a single PayloadPart for a given target.
#[allow(clippy::too_many_arguments)]
fn build_part(
    language: Language,
    target: TargetName,
    base: &Path,
    target_dir: &Path,
    fingerprints: &[RequiredFile],
    all_strings_patch: &payload::StringsPatch,
    all_voice_patch: &payload::VoicePatch,
    runtime_bundled: &[BundledFile],
) -> std::result::Result<PayloadPart, String> {
    let (strings, voice, file_patches, executable_plain, executable_portable, bundled, is_empty) =
        match target {
            TargetName::Sprites => {
                let mut fps = Vec::new();
                for file_name in &["sysfont.dat", "Sprite1.dat", "sprite2.dat", "bitmaps.dat"] {
                    let original = read(base, file_name)?;
                    let localized = read(target_dir, file_name)?;
                    if original != localized {
                        let fp = FilePatch::create(file_name.to_string(), &original, &localized)?;
                        println!(
                            "  {}/{}: {} -> {} bytes (delta {})",
                            target.label(),
                            file_name,
                            original.len(),
                            localized.len(),
                            fp.delta.len()
                        );
                        fps.push(fp);
                    }
                }
                let empty = fps.is_empty();
                (None, None, fps, None, None, Vec::new(), empty)
            }
            TargetName::Runtime => {
                let empty_exe = FilePatch::create("Doraemon.exe".to_string(), &[], &[])?;
                let bundled = runtime_bundled.to_vec();
                let empty = bundled.is_empty();
                (None, None, Vec::new(), None, Some(empty_exe), bundled, empty)
            }
            _ => {
                let strings = if all_strings_patch.replacements.is_empty()
                    || target.string_groups().is_empty()
                {
                    None
                } else {
                    let filtered = payload::filter_strings(all_strings_patch, target);
                    if filtered.replacements.is_empty() {
                        None
                    } else {
                        Some(filtered)
                    }
                };
                let voice = if all_voice_patch.replacements.is_empty()
                    || (target != TargetName::Others && target.character_index().is_none())
                {
                    None
                } else {
                    let filtered = payload::filter_voice(all_voice_patch, target);
                    if filtered.replacements.is_empty() {
                        None
                    } else {
                        Some(filtered)
                    }
                };
                let s_none = strings.is_none();
                let v_none = voice.is_none();
                (strings, voice, Vec::new(), None, None, Vec::new(), s_none && v_none)
            }
        };

    Ok(PayloadPart {
        format_version: 1,
        language,
        target,
        base_fingerprints: fingerprints.to_vec(),
        strings,
        voice,
        file_patches,
        executable_plain,
        executable_portable,
        bundled,
        is_empty,
    })
}

fn release_parts(arguments: &[String]) -> Result<(), String> {
    let language = match value(arguments, "--language").as_deref() {
        Some("english") => Language::English,
        Some("vietnamese") => Language::Vietnamese,
        _ => usage(),
    };
    let base = PathBuf::from(value(arguments, "--base-dir").unwrap_or_else(|| usage()));
    let target_dir = PathBuf::from(value(arguments, "--target-dir").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));

    let target_spec = value(arguments, "--target").unwrap_or_else(|| "all".into());
    let targets: Vec<TargetName> = if target_spec == "all" {
        TargetName::all().to_vec()
    } else {
        match TargetName::from_label(&target_spec) {
            Some(t) => vec![t],
            None => {
                eprintln!(
                    "Error: unknown target '{target_spec}'. Valid: all, doraemon, nobita, dorami, shizuka, suneo, gian, others, sprites, runtime"
                );
                std::process::exit(2);
            }
        }
    };

    fs::create_dir_all(&output).map_err(|error| error.to_string())?;

    let original_exe = read(&base, "Doraemon.exe")?;

    let strings_patch =
        strings::create_patch(&read(&base, "strings.dat")?, &read(&target_dir, "strings.dat")?)?;
    println!(
        "strings.dat: {} records total, {} translated",
        strings_patch.expected_ids.len(),
        strings_patch.replacements.len()
    );

    let voice_patch =
        voice::create_patch(&read(&base, "voice.dat")?, &read(&target_dir, "voice.dat")?)?;
    println!(
        "voice.dat: {} records total, {} changed",
        voice_patch.expected_ids.len(),
        voice_patch.replacements.len()
    );

    pe::patch_language_runtime(
        &original_exe,
        language == Language::Vietnamese,
        false,
        false,
        false,
        false,
    )
    .map_err(|error| format!("unsupported base Doraemon.exe structure: {error}"))?;

    let fingerprints = build_fingerprints(&base)?;
    let runtime_bundled = runtime_files(arguments)?;

    for &target in &targets {
        let part = build_part(
            language,
            target,
            &base,
            &target_dir,
            &fingerprints,
            &strings_patch,
            &voice_patch,
            &runtime_bundled,
        )?;

        let encoded = payload::encode_part(&part)?;
        let part_path = output.join(target.filename());
        fs::write(&part_path, &encoded).map_err(|error| error.to_string())?;
        let empty_flag = if part.is_empty { " [empty]" } else { "" };
        println!(
            "Wrote {} ({} bytes){}",
            part_path.display(),
            encoded.len(),
            empty_flag
        );
    }

    // Also generate the monolithic .dmpatch for backward compatibility
    if target_spec == "all" {
        let full_payload = Payload {
            language,
            profiles: vec![build_profile("Original v1.26", &base, &target_dir)?],
            strings: Some(strings_patch),
            voice: Some(voice_patch),
            bundled: runtime_bundled,
        };
        let encoded = payload::encode(&full_payload)?;
        let compat_path = output.join(format!("{}.dmpatch", language.label().to_ascii_lowercase()));
        fs::write(&compat_path, &encoded).map_err(|error| error.to_string())?;
        println!(
            "Wrote monolithic compat {} ({} bytes)",
            compat_path.display(),
            encoded.len()
        );
    }

    Ok(())
}

fn release(arguments: &[String]) -> Result<(), String> {
    let language = match value(arguments, "--language").as_deref() {
        Some("english") => Language::English,
        Some("vietnamese") => Language::Vietnamese,
        _ => usage(),
    };
    let base = PathBuf::from(value(arguments, "--base-dir").unwrap_or_else(|| usage()));
    let target = PathBuf::from(value(arguments, "--target-dir").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let original_exe = read(&base, "Doraemon.exe")?;
    let strings_patch =
        strings::create_patch(&read(&base, "strings.dat")?, &read(&target, "strings.dat")?)?;
    println!(
        "strings.dat: {} records, {} translated records",
        strings_patch.expected_ids.len(),
        strings_patch.replacements.len()
    );
    let voice_patch =
        voice::create_patch(&read(&base, "voice.dat")?, &read(&target, "voice.dat")?)?;
    println!(
        "voice.dat: {} records, {} changed records",
        voice_patch.expected_ids.len(),
        voice_patch.replacements.len()
    );
    pe::patch_language_runtime(
        &original_exe,
        language == Language::Vietnamese,
        false,
        false,
        false,
        false,
    )
    .map_err(|error| format!("unsupported base Doraemon.exe structure: {error}"))?;
    let profiles = vec![build_profile("Original v1.26", &base, &target)?];
    let payload = Payload {
        language,
        profiles,
        strings: Some(strings_patch),
        voice: Some(voice_patch),
        bundled: if arguments
            .iter()
            .any(|argument| argument == "--payload-only")
        {
            wrapper_files(arguments)?
        } else {
            runtime_files(arguments)?
        },
    };
    let encoded = payload::encode(&payload)?;
    let payload_path = output.join(format!("{}.dmpatch", language.label().to_ascii_lowercase()));
    fs::write(&payload_path, &encoded).map_err(|error| error.to_string())?;
    println!(
        "Wrote {} ({} bytes).",
        payload_path.display(),
        encoded.len()
    );
    if arguments
        .iter()
        .any(|argument| argument == "--payload-only")
    {
        return Ok(());
    }
    let rust_target =
        value(arguments, "--target").unwrap_or_else(|| "x86_64-pc-windows-gnu".into());
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new("cargo")
        .current_dir(&workspace)
        .env(
            "DORAEMON_PATCH_PAYLOAD",
            fs::canonicalize(&payload_path).map_err(|error| error.to_string())?,
        )
        .arg("build")
        .arg("--release")
        .arg("-p")
        .arg("doraemon-patcher")
        .arg("--target")
        .arg(&rust_target)
        .status()
        .map_err(|error| format!("start Cargo: {error}"))?;
    if !status.success() {
        return Err(format!("Windows patcher build failed with {status}"));
    }
    let built = workspace
        .join("target")
        .join(&rust_target)
        .join("release")
        .join("doraemon-patcher.exe");
    let destination = output.join(format!("Doraemon-{}-Patcher.exe", language.label()));
    fs::copy(&built, &destination).map_err(|error| format!("{}: {error}", built.display()))?;
    let checksum = hash::file(&destination)?;
    fs::write(
        output.join(format!("Doraemon-{}-Patcher.exe.sha256", language.label())),
        format!(
            "{}  {}\n",
            hash::hex(&checksum),
            destination.file_name().unwrap().to_string_lossy()
        ),
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        output.join("README.txt"),
        "Doraemon Monopoly localization patcher\r\n\r\nUse only with your own supported Cantonese installation. Copy this Windows 7+ patcher beside Doraemon.exe and run it there. It validates every required file, creates backup\\original, backup\\manifest.json, and backup\\Restore.exe, then installs verified differences. When local music is selected, BGM.dat is reused or built from a verified DoraemonMusic.wav or CUE/BIN. The patched game streams BGM.dat through its original Win95 DirectSound path without a helper DLL. Leaving local music off preserves the original CD/MCI behavior exactly.\r\n",
    )
    .map_err(|error| error.to_string())?;
    fs::remove_file(payload_path).map_err(|error| error.to_string())?;
    println!("Built {}.", destination.display());
    Ok(())
}

fn portable(arguments: &[String]) -> Result<(), String> {
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let patch = FilePatch::create("Doraemon.exe", &[], &[])?;
    let payload = Payload {
        language: Language::Custom,
        profiles: vec![PatchProfile {
            name: "Runtime compatibility scan".into(),
            required: Vec::new(),
            files: Vec::new(),
            executable_plain: Some(patch.clone()),
            executable_portable: patch,
        }],
        strings: None,
        voice: None,
        bundled: runtime_files(arguments)?,
    };
    let payload_path = output.join("portable.dmpatch");
    fs::write(&payload_path, payload::encode(&payload)?).map_err(|e| e.to_string())?;
    let target_triple =
        value(arguments, "--target").unwrap_or_else(|| "x86_64-pc-windows-gnu".into());
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new("cargo")
        .current_dir(&workspace)
        .env(
            "DORAEMON_PATCH_PAYLOAD_ENGLISH",
            fs::canonicalize(&payload_path).map_err(|e| e.to_string())?,
        )
        .env(
            "DORAEMON_PATCH_PAYLOAD_VIETNAMESE",
            fs::canonicalize(&payload_path).map_err(|e| e.to_string())?,
        )
        .args([
            "build",
            "--release",
            "-p",
            "doraemon-patcher",
            "--target",
            &target_triple,
        ])
        .status()
        .map_err(|e| format!("start Cargo: {e}"))?;
    if !status.success() {
        return Err("Windows patcher build failed".into());
    }
    let built = workspace
        .join("target")
        .join(&target_triple)
        .join("release")
        .join("doraemon-patcher.exe");
    let destination = output.join("Doraemon-Portable-Patcher.exe");
    fs::copy(&built, &destination).map_err(|e| e.to_string())?;
    fs::write(
        output.join("Doraemon-Portable-Patcher.exe.sha256"),
        format!(
            "{}  Doraemon-Portable-Patcher.exe\n",
            hash::hex(&hash::file(&destination)?)
        ),
    )
    .map_err(|e| e.to_string())?;
    fs::write(output.join("README.txt"), "Doraemon v1.26 portable compatibility patcher\r\n\r\nCopy this Windows 7+ patcher beside Doraemon.exe, then run it there. It always patches its own folder and does not ask for a game path. Local music uses compressed BGM.dat through the game\'s Win95-compatible DirectSound path and is installed only when its checkbox is selected and a verified source is available. No audio helper DLL is required. A backup is created before writing.\r\n").map_err(|e| e.to_string())?;
    fs::remove_file(payload_path).ok();
    println!("Built {}", destination.display());
    Ok(())
}

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let result = match arguments.first().map(String::as_str) {
        Some("vi-font") => vi_font(&arguments[1..]),
        Some("extract-audio") => extract_audio(&arguments[1..]),
        Some("release") => release(&arguments[1..]),
        Some("materialize") => materialize(&arguments[1..]),
        Some("materialize-parts") => materialize_parts(&arguments[1..]),
        Some("release-parts") => release_parts(&arguments[1..]),
        Some("package") => package(&arguments[1..]),
        Some("universal") => universal(&arguments[1..]),
        Some("portable") => portable(&arguments[1..]),
        _ => usage(),
    };
    if let Err(error) = result {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

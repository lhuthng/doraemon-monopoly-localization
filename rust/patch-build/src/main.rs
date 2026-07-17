use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use doraemon_game_patch::{
    cue, hash,
    payload::{self, BundledFile, FilePatch, Language, PatchProfile, Payload, RequiredFile},
    pe, strings, sysfont,
};

const FILES: &[&str] = &[
    "Doraemon.exe",
    "strings.dat",
    "sysfont.dat",
    "Sprite1.dat",
    "sprite2.dat",
    "bitmaps.dat",
];

const RESOURCE_FILES: &[&str] = &[
    "strings.dat",
    "sysfont.dat",
    "Sprite1.dat",
    "sprite2.dat",
    "bitmaps.dat",
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
];

fn usage() -> ! {
    eprintln!("Usage:\n  patch-build vi-font --input SYSFONT.DAT --output SYSFONT.DAT\n  patch-build extract-audio --cue DORAEMON.CUE --output DoraemonMusic.wav\n  patch-build release --language english|vietnamese --base-dir DIR --target-dir DIR --output-dir DIR [--cnc-ddraw-dir DIR] [--target x86_64-pc-windows-gnu] [--payload-only]\n  patch-build materialize --payload PATCH.dmpatch --base-dir DIR --output-dir DIR\n  patch-build universal --output-dir DIR [--english-payload PATCH.dmpatch] [--vietnamese-payload PATCH.dmpatch] [--cnc-ddraw-dir DIR] [--target x86_64-pc-windows-gnu]");
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
        .filter(|name| *name != "strings.dat")
    {
        let source = read(&base, name)?;
        let rebuilt = match profile.files.iter().find(|patch| patch.name == name) {
            Some(patch) => patch.apply(&source)?,
            None => source,
        };
        fs::write(output.join(name), rebuilt).map_err(|error| format!("write {name}: {error}"))?;
    }
    println!(
        "Materialized {} resource files in {} from {}.",
        RESOURCE_FILES.len(),
        output.display(),
        payload.language.label()
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
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    if english_path.is_none() && vietnamese_path.is_none() {
        return Err("universal needs at least one language payload".into());
    }
    let wrapper = runtime_files(arguments)?;
    let mut english = english_path
        .as_ref()
        .map(|path| payload::decode(&fs::read(path).map_err(|e| e.to_string())?))
        .transpose()?;
    let mut vietnamese = vietnamese_path
        .as_ref()
        .map(|path| payload::decode(&fs::read(path).map_err(|e| e.to_string())?))
        .transpose()?;
    if english
        .as_ref()
        .is_some_and(|payload| payload.language != Language::English)
        || vietnamese
            .as_ref()
            .is_some_and(|payload| payload.language != Language::Vietnamese)
    {
        return Err("universal payload language does not match its option".into());
    }
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
    let target = value(arguments, "--target").unwrap_or_else(|| "x86_64-pc-windows-gnu".into());
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
        "Doraemon universal patcher\r\n\r\nCopy patcher.exe into the folder containing Doraemon.exe, then run it there.\r\nChoose Unchanged, English, or Vietnamese, pick the compatibility options you want, and press Apply.\r\nThe patcher always works on its own folder, creates a backup before writing, and keeps the window open so you can read the log.\r\n",
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

fn audio_helper_file() -> Result<BundledFile, String> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = workspace.join("native/doraudio/doraudio.c");
    let build = workspace.join("target/doraudio");
    fs::create_dir_all(&build).map_err(|error| error.to_string())?;
    let object = build.join("doraudio.o");
    let dll = build.join("doraudio.dll");
    let compile = Command::new("i686-w64-mingw32-gcc")
        .current_dir(&workspace)
        .args([
            "-Os",
            "-D_WIN32_WINNT=0x0400",
            "-ffreestanding",
            "-fno-builtin",
            "-c",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .status()
        .map_err(|error| format!("start 32-bit Windows C compiler: {error}"))?;
    if !compile.success() {
        return Err(format!("doraudio.dll compilation failed with {compile}"));
    }
    let link = Command::new("i686-w64-mingw32-gcc")
        .current_dir(&workspace)
        .args([
            "-shared",
            "-nostdlib",
            "-Wl,--entry,_DllMain@12",
            "-Wl,--subsystem,windows:4.0",
            "-Wl,--kill-at",
            "-s",
            "-o",
        ])
        .arg(&dll)
        .arg(&object)
        .arg("-lkernel32")
        .status()
        .map_err(|error| format!("link doraudio.dll: {error}"))?;
    if !link.success() {
        return Err(format!("doraudio.dll link failed with {link}"));
    }
    let bytes = fs::read(&dll).map_err(|error| format!("{}: {error}", dll.display()))?;
    Ok(BundledFile {
        name: "doraudio.dll".into(),
        hash: hash::bytes(&bytes),
        bytes,
    })
}

fn runtime_files(arguments: &[String]) -> Result<Vec<BundledFile>, String> {
    let mut files = wrapper_files(arguments)?;
    files.push(audio_helper_file()?);
    Ok(files)
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
        if original != localized && *file_name != "Doraemon.exe" {
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
    // Validate the code structure, not the whole-file hash. Timestamps,
    // checksums, resources, overlays, and previously applied compatible hooks
    // do not change whether the runtime instructions are patchable.
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
        "Doraemon Monopoly localization patcher\r\n\r\nUse only with your own supported Cantonese installation. Copy this patcher beside Doraemon.exe and run it there. It validates every required file, creates backup\\original, backup\\manifest.json, and backup\\Restore.exe, then installs verified differences. When local music is selected, Music.dat is reused or built from a verified DoraemonMusic.wav or CUE/BIN. Leaving local music off preserves the original CD/MCI behavior exactly. Builds made with --cnc-ddraw-dir can also add the graphics wrapper from the patcher window.\r\n",
    )
    .map_err(|error| error.to_string())?;
    fs::remove_file(payload_path).map_err(|error| error.to_string())?;
    println!("Built {}.", destination.display());
    Ok(())
}

fn portable(arguments: &[String]) -> Result<(), String> {
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    // Compatibility mode performs structural EXE detection at runtime. The empty
    // placeholder keeps the payload format stable and embeds no game bytes.
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
        .map_err(|e| e.to_string())?;
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
    fs::write(output.join("README.txt"), "Doraemon v1.26 portable compatibility patcher\r\n\r\nCopy this patcher beside Doraemon.exe, then run it there. It always patches its own folder and does not ask for a game path. The patcher detects supported executable layouts and applies only selected registry, disc, local-music, and volume changes. Local music uses Music.dat through DirectSound and is installed only when its checkbox is selected and a verified source is available. A backup is created before writing.\r\n").map_err(|e| e.to_string())?;
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

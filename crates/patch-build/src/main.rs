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
    eprintln!("Usage:\n  patch-build directsound-8bit --input Doraemon.exe --output Doraemon-8bit.exe\n  patch-build voice-quality --input voice.dat --output voice.dat --quality original|high|balanced|compact\n  patch-build release-parts --language english|vietnamese --base-dir DIR --target-dir DIR --output-dir DIR [--target all|dubbing|sprites|runtime]\n  patch-build merge-parts --parts-dir DIR --output PATCH.dmpatch\n  patch-build materialize-parts --parts-dir DIR --base-dir DIR --output-dir DIR\n  patch-build localized-image --language english|vietnamese --no-disc --no-reg [--local-audio] [--modern-volume] --input Doraemon.exe --output Doraemon.exe\n  patch-build music --input BGM.dat|DoraemonMusic.wav|DORAEMON.cue --output BGM.dat\n  patch-build pack --language english|vietnamese --parts-dir DIR --output-dir DIR\n  patch-build universal --output-dir DIR [--english-payload-dir DIR] [--vietnamese-payload-dir DIR] [--cnc-ddraw-dir DIR]\n  (legacy monolithic and multipart inputs remain supported for migration)");
    std::process::exit(2)
}

fn directsound_8bit(arguments: &[String]) -> Result<(), String> {
    let input = PathBuf::from(value(arguments, "--input").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output").unwrap_or_else(|| usage()));
    if input == output {
        return Err("--output must be a separate file so the original executable is preserved".into());
    }
    let original =
        fs::read(&input).map_err(|error| format!("{}: {error}", input.display()))?;
    let patched = pe::patch_primary_directsound_8bit(&original)?;
    if patched == original {
        return Err("the input executable already uses the guarded 8-bit DirectSound format".into());
    }
    fs::write(&output, &patched)
        .map_err(|error| format!("{}: {error}", output.display()))?;
    let verified =
        fs::read(&output).map_err(|error| format!("verify {}: {error}", output.display()))?;
    if verified != patched {
        return Err(format!("{} failed write verification", output.display()));
    }
    println!(
        "Wrote {} without changing {}. Verified 22,050 Hz stereo, 44,100 bytes/sec, 2-byte blocks, and 8-bit samples.",
        output.display(),
        input.display()
    );
    Ok(())
}

fn merge_parts(arguments: &[String]) -> Result<(), String> {
    let parts_dir = PathBuf::from(value(arguments, "--parts-dir").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output").unwrap_or_else(|| usage()));
    let mut parts = Vec::new();
    let targets = if parts_dir.join(TargetName::Dubbing.filename()).exists() {
        TargetName::all()
    } else {
        TargetName::legacy()
    };
    for target in targets {
        let path = parts_dir.join(target.filename());
        let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        parts.push(payload::decode_part(&bytes).map_err(|error| format!("{}: {error}", path.display()))?);
    }
    let merged = doraemon_game_patch::merge_parts(&parts)?;
    fs::write(&output, payload::encode(&merged)?).map_err(|error| error.to_string())?;
    println!("Merged {} into {}.", parts_dir.display(), output.display());
    Ok(())
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

/// Rebuild one resource from a language payload. This is intentionally narrow:
/// it restores a missing ignored Studio file without touching the rest of a
/// contributor's local graphics workspace.
fn materialize_file(arguments: &[String]) -> Result<(), String> {
    let payload_path = PathBuf::from(value(arguments, "--payload").unwrap_or_else(|| usage()));
    let base = PathBuf::from(value(arguments, "--base-dir").unwrap_or_else(|| usage()));
    let name = value(arguments, "--file").unwrap_or_else(|| usage());
    let output = PathBuf::from(value(arguments, "--output").unwrap_or_else(|| usage()));
    if !RESOURCE_FILES.contains(&name.as_str()) {
        return Err(format!("{name} is not a materializable resource file"));
    }
    let payload = payload::decode(&fs::read(&payload_path).map_err(|error| error.to_string())?)?;
    let profile = payload
        .profiles
        .iter()
        .find(|profile| {
            profile.required.iter().all(|required| {
                read(&base, &required.name)
                    .map(|bytes| bytes.len() as u64 == required.len && hash::bytes(&bytes) == required.hash)
                    .unwrap_or(false)
            })
        })
        .ok_or("the supplied folder does not contain a supported original resource set")?;
    let source = read(&base, &name)?;
    let rebuilt = match name.as_str() {
        "strings.dat" => payload
            .strings
            .as_ref()
            .map(|patch| strings::apply_patch(&source, patch))
            .transpose()?
            .unwrap_or(source),
        "voice.dat" => payload
            .voice
            .as_ref()
            .map(|patch| voice::apply_patch(&source, patch))
            .transpose()?
            .unwrap_or(source),
        _ => match profile.files.iter().find(|patch| patch.name == name) {
            Some(patch) => patch.apply(&source)?,
            None => source,
        },
    };
    fs::write(&output, rebuilt).map_err(|error| format!("{}: {error}", output.display()))?;
    println!("Materialized {name} in {}.", output.display());
    Ok(())
}

fn materialize_parts(arguments: &[String]) -> Result<(), String> {
    let parts_dir = PathBuf::from(value(arguments, "--parts-dir").unwrap_or_else(|| usage()));
    let base = PathBuf::from(value(arguments, "--base-dir").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));

    let mut parts = Vec::new();
    let targets = if parts_dir.join(TargetName::Dubbing.filename()).exists() {
        TargetName::all()
    } else {
        TargetName::legacy()
    };
    for target in targets {
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

    let source_strings = read(&base, "strings.dat")?;
    let rebuilt_strings = merged
        .strings
        .as_ref()
        .map(|patch| strings::apply_patch(&source_strings, patch))
        .transpose()?
        .unwrap_or(source_strings);
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
    let rebuilt_voice = merged
        .voice
        .as_ref()
        .map(|patch| voice::apply_patch(&source_voice, patch))
        .transpose()?
        .unwrap_or(source_voice);
    fs::write(output.join("voice.dat"), rebuilt_voice)
        .map_err(|error| format!("write voice.dat: {error}"))?;

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
        "Materialized resources from {} components in {} to {}.",
        parts.len(),
        parts_dir.display(),
        output.display()
    );
    Ok(())
}

/// Writes the executable that belongs in a language bundle.
///
/// Vietnamese needs the CC/CD font dispatch the desktop patcher installs, or its
/// accented text renders as Chinese glyphs; English needs no font change. Both
/// need the two portability patches, and are refused without them: a rehost has
/// no CD drive and no Setup registry entry, so the guest's own checks are lifted
/// here, the same way the patcher's two checkboxes lift them on the desktop.
/// `--local-audio` additionally installs the BGM.dat streaming runtime, since a
/// browser has no MCI CD device either. The routine is the patcher's, so the
/// browser bundle and a desktop install with the same boxes ticked cannot drift.
fn localized_image(arguments: &[String]) -> Result<(), String> {
    let vietnamese = match value(arguments, "--language").as_deref() {
        Some("english") => false,
        Some("vietnamese") => true,
        _ => usage(),
    };
    let ticked = |flag: &str| arguments.iter().any(|argument| argument == flag);
    if !ticked("--no-disc") || !ticked("--no-reg") {
        return Err(
            "a ported image needs --no-disc and --no-reg: the browser has no CD drive and no \
             Setup registry entry, so both of the guest's checks must be lifted"
                .into(),
        );
    }
    let local_audio = ticked("--local-audio");
    // A browser has no mixer to read: the guest's `mixerGetControlDetailsA`
    // fails, so its options screen opens with the SFX knob at zero while new
    // DirectSound buffers are already at full volume. The patcher's modern SFX
    // block answers that read and drives the slider through buffer volume, which
    // is the same fix a desktop port needs on a machine whose mixer is missing.
    let modern_volume = ticked("--modern-volume");
    let input = PathBuf::from(value(arguments, "--input").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output").unwrap_or_else(|| usage()));
    let original =
        fs::read(&input).map_err(|error| format!("{}: {error}", input.display()))?;
    let patched = pe::patch_language_runtime(
        &original,
        vietnamese,
        true,
        true,
        local_audio,
        modern_volume,
    )
    .map_err(|error| format!("unsupported base Doraemon.exe structure: {error}"))?;
    fs::write(&output, &patched.bytes)
        .map_err(|error| format!("write {}: {error}", output.display()))?;
    println!("{}: {}", output.display(), patched.actions.join("; "));
    Ok(())
}

/// Re-encodes `voice.dat` at one of the patcher's quality levels.
///
/// The desktop installer applies this to the copy it installs, from the same
/// UI choice; a browser build has no installer, so the conversion runs here on
/// the tree the localized parts were just materialized into. `compact` is
/// 11 kHz 8-bit stereo, which is what makes a fully dubbed build small enough
/// to serve — the same reduction the installer's "Compact" tick produces.
fn voice_quality(arguments: &[String]) -> Result<(), String> {
    let quality = match value(arguments, "--quality").as_deref() {
        Some("original") => voice::Compression::Original,
        Some("high") => voice::Compression::High,
        Some("balanced") => voice::Compression::Balanced,
        Some("compact") => voice::Compression::Compact,
        _ => usage(),
    };
    let input = PathBuf::from(value(arguments, "--input").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output").unwrap_or_else(|| usage()));
    let source = fs::read(&input).map_err(|error| format!("{}: {error}", input.display()))?;
    let encoded = voice::compress_audio(&source, quality)
        .map_err(|error| format!("{}: {error}", input.display()))?;
    fs::write(&output, &encoded).map_err(|error| format!("write {}: {error}", output.display()))?;
    println!(
        "{}: {} bytes from {} bytes",
        output.display(),
        encoded.len(),
        source.len()
    );
    Ok(())
}

/// Writes the compressed BGM.dat the local-music runtime streams.
///
/// Accepts an already-encoded BGM.dat (verified and copied), the verified
/// DoraemonMusic.wav disc extraction, or a verified CUE/BIN pair. Encoding is
/// the desktop patcher's, so a browser bundle and a desktop install with local
/// music enabled play identical data. `--pcm` emits version 2, raw 16-bit PCM:
/// that is what a browser build wants, because the guest would otherwise spend
/// about one instruction in seven decoding ADPCM inside a timer callback.
fn music(arguments: &[String]) -> Result<(), String> {
    let input = PathBuf::from(value(arguments, "--input").unwrap_or_else(|| usage()));
    let output = PathBuf::from(value(arguments, "--output").unwrap_or_else(|| usage()));
    let pcm = arguments.iter().any(|argument| argument == "--pcm");
    let extension = input
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match (extension.as_str(), pcm) {
        ("dat", false) => {
            if !doraemon_game_patch::music::valid(&input) {
                return Err(format!("{} is not a valid BGM.dat", input.display()));
            }
            fs::copy(&input, &output)
                .map_err(|error| format!("{}: {error}", output.display()))?;
        }
        ("dat", true) => doraemon_game_patch::music::to_pcm(&input, &output)
            .map_err(|error| error.to_string())?,
        ("wav", false) => doraemon_game_patch::music::encode_wav(&input, &output)
            .map_err(|error| error.to_string())?,
        ("wav", true) => doraemon_game_patch::music::encode_wav_pcm(&input, &output)
            .map_err(|error| error.to_string())?,
        ("cue", false) => doraemon_game_patch::music::encode_cue(&input, &output)
            .map_err(|error| error.to_string())?,
        ("cue", true) => doraemon_game_patch::music::encode_cue_pcm(&input, &output)
            .map_err(|error| error.to_string())?,
        _ => {
            return Err(format!(
                "{}: expected BGM.dat, DoraemonMusic.wav, or DORAEMON.cue",
                input.display()
            ))
        }
    }
    println!("{}: {}", output.display(), fs::metadata(&output).map_err(|error| error.to_string())?.len());
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

/// Header prefix for the compressed language packs: four magic bytes followed
/// by the little-endian uncompressed length, then a zstd frame. The patcher
/// reads this exact layout, which is the one it used to embed at build time.
const PACK_MAGIC: &[u8; 4] = b"DZC1";
const PACK_ZSTD_LEVEL: i32 = 19;

fn language_named(name: &str) -> Result<Language, String> {
    match name {
        "english" => Ok(Language::English),
        "vietnamese" => Ok(Language::Vietnamese),
        other => Err(format!(
            "unknown language {other}; expected english or vietnamese"
        )),
    }
}

fn pack_file_name(language: Language) -> &'static str {
    match language {
        Language::English => "english-pack.bin",
        _ => "vietnamese-pack.bin",
    }
}

/// Publishes one language as a single download asset: the same DPART container
/// of `.dmpatch` parts the patcher used to embed, zstd-compressed. Shipping it
/// as a release asset instead of linking it into `patcher.exe` is what keeps
/// the executable small.
fn pack(arguments: &[String]) -> Result<(), String> {
    let language = language_named(&value(arguments, "--language").unwrap_or_else(|| usage()))?;
    let parts_dir = PathBuf::from(value(arguments, "--parts-dir").unwrap_or_else(|| usage()));
    let output_dir = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;

    let targets: Vec<TargetName> = if parts_dir.join(TargetName::Dubbing.filename()).exists() {
        TargetName::all().to_vec()
    } else {
        TargetName::legacy().to_vec()
    };
    let mut part_bytes: Vec<Vec<u8>> = Vec::new();
    let mut parts = Vec::new();
    for target in &targets {
        let path = parts_dir.join(target.filename());
        let bytes = if path.is_file() {
            fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?
        } else {
            println!("{}: missing, packing an empty placeholder", path.display());
            Vec::new()
        };
        if !bytes.is_empty() {
            parts.push(payload::decode_part(&bytes).map_err(|error| format!("{}: {error}", path.display()))?);
        }
        part_bytes.push(bytes);
    }
    if parts.is_empty() {
        return Err(format!("no patch parts found in {}", parts_dir.display()));
    }
    let merged = doraemon_game_patch::merge_parts(&parts)?;
    if merged.language != language {
        return Err(format!(
            "{} holds {} parts, not {}",
            parts_dir.display(),
            merged.language.suffix(),
            language.suffix()
        ));
    }

    // DPART is five magic bytes, a two-byte part count, one offset/length pair
    // per part, then the parts themselves.
    let mut blob = Vec::new();
    blob.extend_from_slice(b"DPART");
    blob.extend_from_slice(&(part_bytes.len() as u16).to_le_bytes());
    let mut data_start = 7 + part_bytes.len() * 8;
    for bytes in &part_bytes {
        blob.extend_from_slice(&(data_start as u32).to_le_bytes());
        blob.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        data_start += bytes.len();
    }
    for bytes in &part_bytes {
        blob.extend_from_slice(bytes);
    }

    let mut compressed = Vec::with_capacity(blob.len() / 2 + 16);
    compressed.extend_from_slice(PACK_MAGIC);
    compressed.extend_from_slice(&(blob.len() as u64).to_le_bytes());
    compressed.extend_from_slice(
        &zstd::bulk::compress(&blob, PACK_ZSTD_LEVEL)
            .map_err(|error| format!("compress pack: {error}"))?,
    );

    let destination = output_dir.join(pack_file_name(language));
    fs::write(&destination, &compressed).map_err(|error| error.to_string())?;
    fs::write(
        output_dir.join(format!("{}.sha256", pack_file_name(language))),
        format!(
            "{}  {}\n",
            hash::hex(&hash::file(&destination)?),
            pack_file_name(language)
        ),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "Wrote {} ({} parts, {:.1} MiB compressed from {:.1} MiB).",
        destination.display(),
        parts.len(),
        compressed.len() as f64 / (1024.0 * 1024.0),
        blob.len() as f64 / (1024.0 * 1024.0)
    );
    Ok(())
}

fn universal(arguments: &[String]) -> Result<(), String> {
    let output = PathBuf::from(value(arguments, "--output-dir").unwrap_or_else(|| usage()));
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let target = value(arguments, "--target").unwrap_or_else(|| "x86_64-pc-windows-gnu".into());
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

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
        .status()
        .map_err(|e| format!("start Cargo: {e}"))?;
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
        "Doraemon universal patcher\r\n\r\nCopy patcher.exe into the folder containing Doraemon.exe, then run it there.\r\nTick <original>, English, or Vietnamese, pick the compatibility options you want, and press Apply.\r\nLanguage packs download once from the release page on first use, then stay cached; patcher.exe itself is small.\r\nThe patcher always works on its own folder, creates a backup before writing, and keeps the window open so you can read the log.\r\n",
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
    // NOTE: sysfont::extend produces BOOTSTRAP glyphs only. The generated
    // variant-0 Vietnamese records stamp marks at guessed positions and most are
    // incorrect; hand-correct them (Font Studio PNG export/import) before this
    // file is used in a release.
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
            || (target != TargetName::Others
                && target != TargetName::Dubbing
                && target.character_index().is_none())
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
                    "Error: unknown target '{target_spec}'. Valid: all, dubbing, sprites, runtime (legacy targets remain available for migration)"
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

    // A monolithic payload is a disposable release artifact, never a tracked
    // collaboration component.
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
        Some("directsound-8bit") => directsound_8bit(&arguments[1..]),
        Some("vi-font") => vi_font(&arguments[1..]),
        Some("extract-audio") => extract_audio(&arguments[1..]),
        Some("release") => release(&arguments[1..]),
        Some("merge-parts") => merge_parts(&arguments[1..]),
        Some("materialize") => materialize(&arguments[1..]),
        Some("materialize-file") => materialize_file(&arguments[1..]),
        Some("materialize-parts") => materialize_parts(&arguments[1..]),
        Some("localized-image") => localized_image(&arguments[1..]),
        Some("voice-quality") => voice_quality(&arguments[1..]),
        Some("music") => music(&arguments[1..]),
        Some("release-parts") => release_parts(&arguments[1..]),
        Some("package") => package(&arguments[1..]),
        Some("pack") => pack(&arguments[1..]),
        Some("universal") => universal(&arguments[1..]),
        Some("portable") => portable(&arguments[1..]),
        _ => usage(),
    };
    if let Err(error) = result {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A browser has neither device, so an image without both lifted checks is
    /// not a ported image, and the step says so instead of writing one.
    #[test]
    fn a_ported_image_requires_both_checks_lifted() {
        let arguments: Vec<String> = [
            "--language",
            "english",
            "--input",
            "missing.exe",
            "--output",
            "out.exe",
        ]
        .iter()
        .map(|argument| argument.to_string())
        .collect();
        let error = localized_image(&arguments).expect_err("the step must refuse");
        assert!(error.contains("--no-disc"), "{error}");
        assert!(error.contains("--no-reg"), "{error}");
    }
}

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use doraemon_game_patch::{
    cue, hash,
    payload::{self, FilePatch, Language, Payload, RequiredFile},
    pe, sysfont,
};

const FILES: &[&str] = &[
    "Doraemon.exe",
    "strings.dat",
    "sysfont.dat",
    "Sprite1.dat",
    "sprite2.dat",
    "bitmaps.dat",
];

const SUPPORTED: &[(&str, &str)] = &[
    (
        "Doraemon.exe",
        "fdf00e681671f93b09d257f77d7ce0720e7129cf6bc44ba9e0f19c2efa4fecba",
    ),
    (
        "strings.dat",
        "9ecce72afcef20e472d70d5d3b642887202ea85fc05e2e22aa05694de972dcec",
    ),
    (
        "sysfont.dat",
        "d4a9885a0358740427d7e5fdfb44d0c9d77a2acab8623549e0a5e4ac5def4510",
    ),
    (
        "Sprite1.dat",
        "3272fa334ad168126b8ea07983f74a2f0e51196c8bf71f181128d9e6f13eb7c4",
    ),
    (
        "sprite2.dat",
        "ec2a6bd53fbced5fc1be29d95e8c1e57729c0b5c4ed3fd4e9fd11694e4123ecc",
    ),
    (
        "bitmaps.dat",
        "9bba998ed68836a2db00316a2df51901533032025a7178a1f5ea560c8d5b63c0",
    ),
];

fn usage() -> ! {
    eprintln!("Usage:\n  patch-build vi-font --input SYSFONT.DAT --output SYSFONT.DAT\n  patch-build extract-audio --cue DORAEMON.CUE --output DoraemonMusic.wav\n  patch-build release --language english|vietnamese --base-dir DIR --target-dir DIR --output-dir DIR [--target x86_64-pc-windows-gnu] [--payload-only]");
    std::process::exit(2)
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
    let mut required = Vec::new();
    let mut patches = Vec::new();
    for name in FILES {
        let original = read(&base, name)?;
        let localized = read(&target, name)?;
        let expected = SUPPORTED
            .iter()
            .find(|(candidate, _)| candidate == name)
            .unwrap()
            .1;
        let actual = hash::hex(&hash::bytes(&original));
        if actual != expected {
            return Err(format!(
                "unsupported base {name}: SHA-256 {actual}; expected {expected}"
            ));
        }
        required.push(RequiredFile {
            name: (*name).into(),
            hash: hash::bytes(&original),
            len: original.len() as u64,
        });
        if original != localized && *name != "Doraemon.exe" {
            let patch = FilePatch::create(*name, &original, &localized)?;
            println!(
                "{}: {} -> {} bytes (delta {})",
                name,
                original.len(),
                localized.len(),
                patch.delta.len()
            );
            patches.push(patch);
        }
    }
    let original_exe = read(&base, "Doraemon.exe")?;
    let (plain, portable) = pe::build_variants(&original_exe, language == Language::Vietnamese)?;
    let payload = Payload {
        language,
        required,
        files: patches,
        executable_plain: plain
            .as_ref()
            .map(|bytes| FilePatch::create("Doraemon.exe", &original_exe, bytes))
            .transpose()?,
        executable_portable: FilePatch::create("Doraemon.exe", &original_exe, &portable)?,
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
        "Doraemon Monopoly localization patchers\r\n\r\nUse only with your own supported Cantonese installation. Select the folder containing Doraemon.exe. The patcher validates every required file, creates backup\\original, backup\\manifest.json, and backup\\Restore.exe, then installs verified differences. No-disc mode can extract DoraemonMusic.wav from your original CUE/BIN. If no valid audio is supplied, the game continues silently.\r\n",
    )
    .map_err(|error| error.to_string())?;
    fs::remove_file(payload_path).map_err(|error| error.to_string())?;
    println!("Built {}.", destination.display());
    Ok(())
}

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let result = match arguments.first().map(String::as_str) {
        Some("vi-font") => vi_font(&arguments[1..]),
        Some("extract-audio") => extract_audio(&arguments[1..]),
        Some("release") => release(&arguments[1..]),
        _ => usage(),
    };
    if let Err(error) = result {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

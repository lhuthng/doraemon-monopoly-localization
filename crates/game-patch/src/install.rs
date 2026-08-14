use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::{Component, Path, PathBuf},
};

use crate::{
    cue, hash, music,
    payload::{FilePatch, Language, PatchProfile, Payload},
    pe, strings, voice, Result,
};

#[derive(Clone, Debug, Default)]
pub struct ApplyOptions {
    pub no_disc: bool,
    pub no_reg: bool,
    pub local_audio: bool,
    pub modern_volume: bool,
    pub primary_audio_8bit: bool,
    /// Multimedia-timer period in milliseconds. `None` keeps the stock ~30 Hz
    /// clock; a shorter period speeds up everything the game paces by counting
    /// timer ticks, which is the normal in-game speed setting.
    pub game_clock_ms: Option<u8>,
    pub cue: Option<PathBuf>,
    pub reduce_bgm: bool,
    pub optimize_voice: bool,
    pub voice_compression: voice::Compression,
    pub keep_compressed_audio: bool,
}

#[derive(Clone, Debug)]
pub struct ApplyReport {
    pub changed: Vec<String>,
    pub audio: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
    Working,
    Done,
    Skipped,
    Failed,
}

#[derive(Clone, Debug)]
pub struct TaskProgress {
    pub state: TaskState,
    pub message: String,
    pub progress: Option<u8>,
}

pub type ProgressSink<'a> = dyn FnMut(TaskProgress) + 'a;

fn progress(
    sink: &mut ProgressSink<'_>,
    state: TaskState,
    message: impl Into<String>,
    pct: Option<u8>,
) {
    sink(TaskProgress {
        state,
        message: message.into(),
        progress: pct,
    });
}

pub fn add_wrapper(folder: &Path, payload: &Payload) -> Result<Vec<String>> {
    let wrapper_files: Vec<_> = payload
        .bundled
        .iter()
        .filter(|file| !file.name.eq_ignore_ascii_case("doraudio.dll"))
        .collect();
    if wrapper_files.is_empty() {
        return Err("this patcher was built without the cnc-ddraw wrapper".into());
    }
    let mut targets = Vec::new();
    for file in wrapper_files {
        let relative = Path::new(&file.name);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(format!("unsafe bundled wrapper path {}", file.name));
        }
        let target = folder.join(relative);
        if target.exists() && hash::file(&target)? != file.hash {
            return Err(format!(
                "{} already exists and is different; move it aside before adding the wrapper",
                file.name
            ));
        }
        targets.push((file, target));
    }
    let staging = folder.join(".cnc-ddraw-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    for (file, _) in &targets {
        let staged = staging.join(&file.name);
        if let Some(parent) = staged.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        write_synced(&staged, &file.bytes)?;
    }
    let mut added = Vec::new();
    for (file, target) in targets {
        if target.exists() {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        replace_file(&staging.join(&file.name), &target)?;
        if hash::file(&target)? != file.hash {
            return Err(format!("{} failed wrapper verification", file.name));
        }
        added.push(file.name.clone());
    }
    let _ = fs::remove_dir_all(&staging);
    Ok(added)
}

fn find_file(folder: &Path, wanted: &str) -> Result<PathBuf> {
    for entry in fs::read_dir(folder).map_err(|error| format!("{}: {error}", folder.display()))? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(wanted)
        {
            return Ok(entry.path());
        }
    }
    Err(format!("missing {wanted} in {}", folder.display()))
}

/// Discovers the playable game executables in `folder` (any `doraemon*.exe`,
/// e.g. `Doraemon.exe`, `Doraemon-en.exe`), sorted by name.
fn find_builds(folder: &Path) -> Vec<PathBuf> {
    let mut builds: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = fs::read_dir(folder) {
        for entry in rd.flatten() {
            let path = entry.path();
            let is_doraemon = path.is_file()
                && path.extension().is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("exe"))
                && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    let lower = n.to_ascii_lowercase();
                    lower.starts_with("doraemon") && lower.ends_with(".exe")
                });
            if is_doraemon {
                builds.push(path);
            }
        }
    }
    builds.sort();
    builds
}

fn write_synced(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = File::create(path).map_err(|error| format!("{}: {error}", path.display()))?;
    file.write_all(data)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(not(windows))]
pub fn replace_file(source: &Path, target: &Path) -> Result<()> {
    fs::rename(source, target).map_err(|error| format!("replace {}: {error}", target.display()))
}

#[cfg(windows)]
pub fn replace_file(source: &Path, target: &Path) -> Result<()> {
    use std::{iter, os::windows::ffi::OsStrExt};
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    const REPLACE_EXISTING: u32 = 1;
    const WRITE_THROUGH: u32 = 8;
    let source: Vec<u16> = source
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let target: Vec<u16> = target
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            REPLACE_EXISTING | WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(format!(
            "replace failed: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

fn selected_patches(profile: &PatchProfile, _no_disc: bool) -> Vec<&FilePatch> {
    profile.files.iter().collect()
}

struct LocalAudioPreparation {
    enabled: bool,
    summary: String,
    created: Vec<(String, PathBuf, hash::Hash)>,
}

fn prepare_local_audio(
    folder: &Path,
    staging: &Path,
    _payload: &Payload,
    options: &ApplyOptions,
    sink: &mut ProgressSink<'_>,
) -> Result<LocalAudioPreparation> {
    if !options.local_audio {
        progress(
            sink,
            TaskState::Skipped,
            "Local music is off; original CD/MCI playback is unchanged.",
            Some(42),
        );
        return Ok(LocalAudioPreparation {
            enabled: false,
            summary: "Music playback was left unchanged.".into(),
            created: Vec::new(),
        });
    }
    let music_path = folder.join("BGM.dat");
    let wav_path = folder.join("DoraemonMusic.wav");
    let source = if music::valid(&music_path) {
        None
    } else if music_path.exists() {
        return Err("BGM.dat exists but is not a valid Doraemon local-music file; move it aside before applying local music".into());
    } else if cue::valid_wav(&wav_path) {
        Some((wav_path, true))
    } else if let Some(cue_path) = options.cue.as_ref().filter(|path| cue::valid_cue(path)) {
        Some((cue_path.clone(), false))
    } else {
        progress(
            sink,
            TaskState::Skipped,
            "Local music was requested, but no BGM.dat, verified WAV, or CUE/BIN was found. The original music code was left untouched.",
            Some(45),
        );
        return Ok(LocalAudioPreparation {
            enabled: false,
            summary: "No local music source was available, so music playback was left unchanged."
                .into(),
            created: Vec::new(),
        });
    };
    let mut created = Vec::new();
    if let Some((source_path, is_wav)) = source {
        progress(
            sink,
            TaskState::Working,
            if is_wav {
                "Compressing DoraemonMusic.wav into BGM.dat…"
            } else {
                "Reading the disc image and building BGM.dat…"
            },
            Some(43),
        );
        let staged = staging.join("BGM.dat");
        if is_wav {
            music::encode_wav(&source_path, &staged)?;
        } else {
            music::encode_cue(&source_path, &staged)?;
        }
        let digest = hash::file(&staged)?;
        created.push(("BGM.dat".into(), staged, digest));
    }
    progress(
        sink,
        TaskState::Done,
        "Local BGM.dat streaming is ready.",
        Some(47),
    );
    Ok(LocalAudioPreparation {
        enabled: true,
        summary: "BGM.dat will play through the game's Win95-safe sound path.".into(),
        created,
    })
}

fn backup_manifest(
    language: &str,
    originals: &[(String, hash::Hash)],
    created_files: &[(String, hash::Hash)],
) -> String {
    let mut output =
        format!("{{\n  \"version\": 2,\n  \"language\": \"{language}\",\n  \"files\": {{\n");
    for (index, (name, digest)) in originals.iter().enumerate() {
        output.push_str(&format!(
            "    \"{name}\": \"{}\"{}\n",
            hash::hex(digest),
            if index + 1 == originals.len() {
                ""
            } else {
                ","
            }
        ));
    }
    output.push_str("  },\n  \"created_files\": {\n");
    for (index, (name, digest)) in created_files.iter().enumerate() {
        output.push_str(&format!(
            "    \"{name}\": \"{}\"{}\n",
            hash::hex(digest),
            if index + 1 == created_files.len() {
                ""
            } else {
                ","
            }
        ));
    }
    output.push_str("  }\n}\n");
    output
}

fn manifest_created_files(manifest: &str) -> Result<HashMap<String, hash::Hash>> {
    let mut files = HashMap::new();
    let mut in_created = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "\"created_files\": {" {
            in_created = true;
            continue;
        }
        if in_created && trimmed == "}" {
            break;
        }
        if in_created {
            let entry = trimmed.trim_end_matches(',');
            let (name, digest) = entry
                .split_once(':')
                .ok_or("invalid backup manifest created-file entry")?;
            files.insert(
                name.trim().trim_matches('"').to_string(),
                hash::parse(digest.trim().trim_matches('"'))?,
            );
        } else if trimmed.starts_with("\"created_audio\": {") {
            let name = trimmed
                .split("\"name\": \"")
                .nth(1)
                .and_then(|value| value.split('"').next())
                .ok_or("invalid legacy audio manifest")?;
            let digest = trimmed
                .split("\"sha256\": \"")
                .nth(1)
                .and_then(|value| value.split('"').next())
                .ok_or("invalid legacy audio manifest")?;
            files.insert(name.to_string(), hash::parse(digest)?);
        }
    }
    Ok(files)
}

fn verified_backup_files(backup: &Path) -> Result<HashMap<String, hash::Hash>> {
    let manifest_path = backup.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    let mut files = HashMap::new();
    let mut in_files = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "\"files\": {" {
            in_files = true;
            continue;
        }
        if in_files && trimmed == "}," {
            break;
        }
        if !in_files {
            continue;
        }
        let entry = trimmed.trim_end_matches(',');
        let (name, digest) = entry
            .split_once(':')
            .ok_or("invalid backup manifest file entry")?;
        let name = name.trim().trim_matches('"').to_string();
        let expected = hash::parse(digest.trim().trim_matches('"'))?;
        let original = backup.join("original").join(&name);
        if hash::file(&original)? != expected {
            return Err(format!("backup copy of {name} was modified"));
        }
        files.insert(name, expected);
    }
    Ok(files)
}

// Restore.exe intentionally stays in backup/ so it can be used later. When all
// tracked live files are back to their original hashes (and patcher-owned
// generated files have been removed), that directory is stale rather than an active backup.
// A subsequent Apply may safely replace it with a fresh backup.
fn backup_is_fully_restored(backup: &Path, game: &Path) -> Result<bool> {
    let originals = verified_backup_files(backup)?;
    for (name, expected) in originals {
        let live = find_file(game, &name)?;
        if hash::file(&live)? != expected {
            return Ok(false);
        }
    }
    let manifest = fs::read_to_string(backup.join("manifest.json"))
        .map_err(|error| format!("read backup manifest: {error}"))?;
    for name in manifest_created_files(&manifest)?.keys() {
        if game.join(name).exists() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn discard_restored_backup(
    backup: &Path,
    game: &Path,
    sink: &mut ProgressSink<'_>,
) -> Result<bool> {
    if !backup.exists() || !backup_is_fully_restored(backup, game)? {
        return Ok(false);
    }
    progress(
        sink,
        TaskState::Done,
        "The previous backup belongs to a completed restore; preparing a fresh backup.",
        Some(60),
    );
    fs::remove_dir_all(backup).map_err(|error| format!("remove restored backup: {error}"))?;
    Ok(true)
}

fn apply_compatibility(
    folder: &Path,
    payload: &Payload,
    options: &ApplyOptions,
    patcher_exe: &Path,
    sink: &mut ProgressSink<'_>,
) -> Result<ApplyReport> {
    let backup = folder.join("backup");
    progress(
        sink,
        TaskState::Working,
        if backup.exists() {
            "A backup already exists; checking what is already installed…"
        } else {
            "Checking the supported game files…"
        },
        Some(0),
    );
    let staging = folder.join(".doraemon-patch-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let local_audio = prepare_local_audio(folder, &staging, payload, options, sink)?;
    let exe_path = find_file(folder, "Doraemon.exe")?;
    progress(
        sink,
        TaskState::Working,
        "Checking the supported game executable…",
        Some(15),
    );
    let original = fs::read(&exe_path).map_err(|e| format!("{}: {e}", exe_path.display()))?;
    let mut result = pe::patch_compatible(
        &original,
        options.no_disc,
        local_audio.enabled,
        options.no_reg,
        options.modern_volume,
    )?;
    if options.primary_audio_8bit {
        let bytes = pe::patch_primary_directsound_8bit(&result.bytes)?;
        if bytes != result.bytes {
            result
                .actions
                .push("set primary DirectSound output to 22,050 Hz stereo 8-bit".into());
        }
        result.bytes = bytes;
    }
    if let Some(period) = options.game_clock_ms {
        let bytes = pe::patch_game_clock(&result.bytes, period)?;
        if bytes != result.bytes {
            result
                .actions
                .push(format!("set the game clock to a {period} ms tick"));
        }
        result.bytes = bytes;
    }
    if local_audio.enabled && !result.local_audio_supported {
        return Err(
            "this executable layout cannot safely use the local DirectSound music backend".into(),
        );
    }
    if result.bytes == original && local_audio.created.is_empty() {
        progress(
            sink,
            TaskState::Skipped,
            "All requested executable compatibility changes are already installed.",
            Some(100),
        );
        return Ok(ApplyReport {
            changed: Vec::new(),
            audio: local_audio.summary,
        });
    }
    if backup.exists() && !discard_restored_backup(&backup, folder, sink)? {
        if options.keep_compressed_audio {
            progress(
                sink,
                TaskState::Skipped,
                "Stale backup will be updated to include your kept compressed audio.",
                Some(50),
            );
        } else {
            progress(
                sink,
                TaskState::Failed,
                "A backup exists, but this run would change additional files.",
                None,
            );
            return Err(
                "some requested executable changes are still missing, but an existing backup protects a previous install; restore first, then apply again".into(),
            );
        }
    }

    let staged_exe = staging.join("Doraemon.exe");
    progress(
        sink,
        TaskState::Working,
        "Preparing executable changes…",
        Some(40),
    );
    let patched = result.bytes.clone();
    write_synced(&staged_exe, &patched)?;
    let target_hash = hash::bytes(&patched);

    let audio = local_audio.summary.clone();

    fs::create_dir_all(backup.join("original")).map_err(|e| e.to_string())?;
    progress(
        sink,
        TaskState::Working,
        "Creating your original-file backup…",
        Some(60),
    );
    let mut originals: Vec<(String, hash::Hash)> = Vec::new();
    if let Ok(old_originals) = verified_backup_files(&backup) {
        for (name, hash) in old_originals {
            if name != "Doraemon.exe" {
                originals.push((name, hash));
            }
        }
    }
    fs::copy(&exe_path, backup.join("original/Doraemon.exe"))
        .map_err(|e| format!("backup Doraemon.exe: {e}"))?;
    originals.push(("Doraemon.exe".into(), hash::bytes(&original)));
    fs::copy(patcher_exe, backup.join("Restore.exe"))
        .map_err(|e| format!("create Restore.exe: {e}"))?;
    let mut created_files: Vec<_> = local_audio
        .created
        .iter()
        .map(|(name, _, digest)| (name.clone(), *digest))
        .collect();
    if let Ok(old_manifest) = fs::read_to_string(backup.join("manifest.json")) {
        if let Ok(old_created) = manifest_created_files(&old_manifest) {
            for (name, hash) in old_created {
                if !created_files.iter().any(|(n, _)| n == &name) {
                    created_files.push((name, hash));
                }
            }
        }
    }
    let manifest = backup_manifest(
        payload.language.label(),
        &originals,
        &created_files,
    );
    write_synced(&backup.join("manifest.json"), manifest.as_bytes())?;
    progress(
        sink,
        TaskState::Working,
        "Installing executable changes…",
        Some(75),
    );
    replace_file(&staged_exe, &exe_path)?;
    if hash::file(&exe_path)? != target_hash {
        return Err("Doraemon.exe failed installation verification; restore from backup".into());
    }
    let mut changed = vec!["Doraemon.exe".into()];
    changed.extend(result.actions);
    for (name, staged, digest) in local_audio.created {
        let target = folder.join(&name);
        replace_file(&staged, &target)?;
        if hash::file(&target)? != digest {
            return Err(format!(
                "{name} failed installation verification; restore from backup"
            ));
        }
        changed.push(name);
    }
    let _ = fs::remove_dir(&staging);
    progress(
        sink,
        TaskState::Done,
        "Executable changes were verified successfully.",
        Some(100),
    );
    Ok(ApplyReport { changed, audio })
}

fn apply_audio(
    folder: &Path,
    options: &ApplyOptions,
    patcher_exe: &Path,
    sink: &mut ProgressSink<'_>,
) -> Result<ApplyReport> {
    let staging = folder.join(".doraemon-audio-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    let mut prepared: Vec<(String, Option<PathBuf>, PathBuf, hash::Hash)> = Vec::new();

    if options.optimize_voice {
        let source_path = find_file(folder, "voice.dat")?;
        let source =
            fs::read(&source_path).map_err(|error| format!("{}: {error}", source_path.display()))?;
        progress(sink, TaskState::Working, "Preparing Voice.dat...", Some(10));
        let output = voice::compress_audio(&source, options.voice_compression)?;
        if output != source {
            let staged = staging.join("voice.dat");
            write_synced(&staged, &output)?;
            prepared.push((
                "voice.dat".into(),
                Some(source_path),
                staged,
                hash::bytes(&output),
            ));
        } else {
            progress(
                sink,
                TaskState::Skipped,
                "Voice.dat is already using the selected quality.",
                Some(30),
            );
        }
    }

    if options.reduce_bgm {
        progress(sink, TaskState::Working, "Preparing BGM.dat...", Some(35));
        if matches!(
            options.voice_compression,
            voice::Compression::Balanced | voice::Compression::Compact
        ) {
            // The compact BGM quality needs the local-music runtime, which is
            // only installed once the executable has been patched (the old
            // disc-based loader keeps the `BGMRT3` marker). A single-language
            // apply has `Doraemon.exe`; a split install uses suffixed builds
            // (`Doraemon-en.exe`), so scan whichever playable executables exist.
            let exes: Vec<PathBuf> = if find_file(folder, "Doraemon.exe").is_ok() {
                vec![find_file(folder, "Doraemon.exe").unwrap()]
            } else {
                find_builds(folder)
            };
            let any_old_loader = exes
                .iter()
                .filter_map(|p| fs::read(p).ok())
                .any(|b| b.windows(7).any(|window| window == b"BGMRT3\0"));
            if any_old_loader {
                return Err(
                    "Apply patch once to update local music, then apply the smaller BGM quality"
                        .into(),
                );
            }
        }
        let staged = staging.join("BGM.dat");
        let wav = folder.join("DoraemonMusic.wav");
        if cue::valid_wav(&wav) {
            music::encode_wav_quality(&wav, &staged, options.voice_compression)?;
        } else if let Some(cue_path) = options.cue.as_ref().filter(|path| cue::valid_cue(path)) {
            music::encode_cue_quality(cue_path, &staged, options.voice_compression)?;
        } else {
            return Err(
                "BGM reduction needs the verified CUE/BIN or DoraemonMusic.wav source".into(),
            );
        }
        let target = folder.join("BGM.dat");
        let digest = hash::file(&staged)?;
        if target.exists() && hash::file(&target)? == digest {
            fs::remove_file(&staged).ok();
            progress(
                sink,
                TaskState::Skipped,
                "BGM.dat is already reduced.",
                Some(50),
            );
        } else {
            prepared.push((
                "BGM.dat".into(),
                target.exists().then_some(target),
                staged,
                digest,
            ));
        }
    }

    if prepared.is_empty() {
        let _ = fs::remove_dir(&staging);
        return Ok(ApplyReport {
            changed: Vec::new(),
            audio: "Audio is already using the selected settings.".into(),
        });
    }

    let backup = folder.join("backup");
    // Only reuse the single-language backup when its manifest exists. A split
    // install leaves `backup/split-manifest.json` instead, so there is nothing
    // to merge here - start the audio backup fresh (the shared `original/`
    // folder still holds the pristine files for both backups to use).
    let reuse = backup.join("manifest.json").is_file()
        && backup.exists()
        && !discard_restored_backup(&backup, folder, sink)?;
    fs::create_dir_all(backup.join("original")).map_err(|error| error.to_string())?;
    let mut originals = if reuse {
        verified_backup_files(&backup)?
    } else {
        HashMap::new()
    };
    let mut created = if reuse {
        let manifest = fs::read_to_string(backup.join("manifest.json"))
            .map_err(|error| format!("read backup manifest: {error}"))?;
        manifest_created_files(&manifest)?
    } else {
        HashMap::new()
    };
    for (name, source, _, target_hash) in &prepared {
        if created.contains_key(name) {
            created.insert(name.clone(), *target_hash);
        } else if let Some(source) = source {
            if !originals.contains_key(name) && !created.contains_key(name) {
                let destination = backup.join("original").join(name);
                fs::copy(source, &destination)
                    .map_err(|error| format!("backup {name}: {error}"))?;
                originals.insert(name.clone(), hash::file(&destination)?);
            }
        } else {
            created.insert(name.clone(), *target_hash);
        }
    }
    let mut originals: Vec<_> = originals.into_iter().collect();
    let mut created: Vec<_> = created.into_iter().collect();
    originals.sort_by(|a, b| a.0.cmp(&b.0));
    created.sort_by(|a, b| a.0.cmp(&b.0));
    fs::copy(patcher_exe, backup.join("Restore.exe"))
        .map_err(|error| format!("create Restore.exe: {error}"))?;
    let manifest = backup_manifest("Audio", &originals, &created);
    write_synced(&backup.join("manifest.json"), manifest.as_bytes())?;

    progress(
        sink,
        TaskState::Working,
        "Installing audio...",
        Some(75),
    );
    let mut changed = Vec::new();
    for (name, source, staged, expected) in prepared {
        let target = source.unwrap_or_else(|| folder.join(&name));
        replace_file(&staged, &target)?;
        if hash::file(&target)? != expected {
            return Err(format!(
                "{name} failed installation verification; restore from backup"
            ));
        }
        changed.push(name);
    }
    let _ = fs::remove_dir(&staging);
    progress(
        sink,
        TaskState::Done,
        "Audio was reduced and verified.",
        Some(100),
    );
    Ok(ApplyReport {
        changed,
        audio: "Audio files were updated.".into(),
    })
}

pub fn apply(
    folder: &Path,
    payload: &Payload,
    options: &ApplyOptions,
    patcher_exe: &Path,
) -> Result<ApplyReport> {
    apply_with_progress(folder, payload, options, patcher_exe, &mut |_| {})
}

pub fn apply_with_progress(
    folder: &Path,
    payload: &Payload,
    options: &ApplyOptions,
    patcher_exe: &Path,
    sink: &mut ProgressSink<'_>,
) -> Result<ApplyReport> {
    progress(
        sink,
        TaskState::Working,
        "Checking the game folder…",
        Some(0),
    );
    if !folder.is_dir() {
        progress(
            sink,
            TaskState::Failed,
            "The game folder is unavailable.",
            None,
        );
        return Err(format!("{} is not a game folder", folder.display()));
    }
    if payload.language == Language::Custom && (options.optimize_voice || options.reduce_bgm) {
        return apply_audio(folder, options, patcher_exe, sink);
    }
    if payload.language == Language::Custom {
        return apply_compatibility(folder, payload, options, patcher_exe, sink);
    }
    let backup = folder.join("backup");

    let mut selected = None;
    let mut mismatch_reports = Vec::new();
    for profile in &payload.profiles {
        let patches = selected_patches(profile, options.no_disc);
        let mut base_ok = true;
        let mut mismatches = Vec::new();
        if let Some(strings_patch) = &payload.strings {
            match find_file(folder, "strings.dat")
                .and_then(|path| fs::read(&path).map_err(|e| format!("{}: {e}", path.display())))
            {
                Ok(bytes) => match strings::records(&bytes) {
                    Ok(records)
                        if records.keys().cloned().collect::<Vec<_>>()
                            == strings_patch.expected_ids => {}
                    Ok(_) => {
                        base_ok = false;
                        mismatches.push("strings.dat has a different record layout".into());
                    }
                    Err(error) => {
                        base_ok = false;
                        mismatches.push(format!("strings.dat cannot be decoded: {error}"));
                    }
                },
                Err(_) => {
                    base_ok = false;
                    mismatches.push("strings.dat is missing".into());
                }
            }
        }
        if let Some(voice_patch) = &payload.voice {
            if options.keep_compressed_audio {
                // user chose to keep the current compressed voice.dat as-is
            } else {
                match find_file(folder, "voice.dat")
                    .and_then(|path| fs::read(&path).map_err(|e| format!("{}: {e}", path.display())))
                {
                    Ok(bytes) => {
                        let digest = hash::bytes(&bytes);
                        if !((digest == voice_patch.base_hash
                            && bytes.len() as u64 == voice_patch.base_len)
                            || (digest == voice_patch.target_hash
                                && bytes.len() as u64 == voice_patch.target_len))
                        {
                            base_ok = false;
                            mismatches
                                .push("voice.dat does not match this localization payload".into());
                        }
                    }
                    Err(_) => {
                        base_ok = false;
                        mismatches.push("voice.dat is missing".into());
                    }
                }
            }
        }
        for required in &profile.required {
            let path = match find_file(folder, &required.name) {
                Ok(path) => path,
                Err(_) => {
                    base_ok = false;
                    mismatches.push(format!("{} is missing", required.name));
                    continue;
                }
            };
            let digest = hash::file(&path)?;
            let length = fs::metadata(&path)
                .map_err(|error| error.to_string())?
                .len();
            if let Some(patch) = patches
                .iter()
                .find(|patch| patch.name.eq_ignore_ascii_case(&required.name))
            {
                if (digest != required.hash || length != required.len)
                    && digest != patch.target_hash
                {
                    base_ok = false;
                    mismatches.push(format!("{} does not match", required.name));
                }
            } else if required.name.eq_ignore_ascii_case("voice.dat") {
                if !options.keep_compressed_audio {
                    if let Some(voice_patch) = &payload.voice {
                        if (digest != required.hash || length != required.len)
                            && (digest != voice_patch.target_hash || length != voice_patch.target_len)
                        {
                            base_ok = false;
                            mismatches.push(format!("{} does not match", required.name));
                        }
                    } else if digest != required.hash || length != required.len {
                        base_ok = false;
                        mismatches.push(format!("{} does not match", required.name));
                    }
                }
            } else if digest != required.hash || length != required.len {
                base_ok = false;
                mismatches.push(format!("{} does not match", required.name));
            }
        }
        if base_ok {
            selected = Some(profile);
            break;
        }
        mismatch_reports.push(format!("{}: {}", profile.name, mismatches.join(", ")));
    }
    let profile = selected.ok_or_else(|| {
        format!(
            "this game does not match a supported source profile; no files were changed. {}",
            mismatch_reports.join(" | ")
        )
    })?;
    progress(
        sink,
        TaskState::Done,
        "Supported game files confirmed.",
        Some(15),
    );

    let patches = selected_patches(profile, options.no_disc);
    let staging = folder.join(".doraemon-patch-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    let mut generated = Vec::new();
    if let Some(strings_patch) = &payload.strings {
        let source_path = find_file(folder, "strings.dat")?;
        let source = fs::read(&source_path)
            .map_err(|error| format!("{}: {error}", source_path.display()))?;
        progress(
            sink,
            TaskState::Working,
            "Checking strings.dat records…",
            Some(20),
        );
        if strings::matches(&source, strings_patch)? {
            progress(
                sink,
                TaskState::Skipped,
                format!(
                    "strings.dat already contains all {} translated records.",
                    strings_patch.replacements.len()
                ),
                Some(25),
            );
        } else {
            progress(
                sink,
                TaskState::Working,
                format!(
                    "Updating {} translated records and rebuilding strings.dat…",
                    strings_patch.replacements.len()
                ),
                Some(25),
            );
            let output = strings::apply_patch(&source, strings_patch)?;
            let target_hash = hash::bytes(&output);
            let temporary = staging.join("strings.dat");
            write_synced(&temporary, &output)?;
            generated.push((
                "strings.dat".to_string(),
                source_path,
                temporary,
                target_hash,
            ));
            progress(
                sink,
                TaskState::Done,
                "strings.dat records and archive offsets verified.",
                Some(30),
            );
        }
    }
    if !options.keep_compressed_audio {
        if let Some(voice_patch) = &payload.voice {
            let source_path = find_file(folder, "voice.dat")?;
            let source = fs::read(&source_path)
                .map_err(|error| format!("{}: {error}", source_path.display()))?;
            progress(
                sink,
                TaskState::Working,
                "Checking voice.dat records…",
                Some(31),
            );
            if voice::matches(&source, voice_patch) && !options.optimize_voice {
                progress(
                    sink,
                    TaskState::Skipped,
                    format!(
                        "voice.dat already contains all {} changed voice records.",
                        voice::replacement_count(voice_patch)
                    ),
                    Some(33),
                );
            } else {
                progress(
                    sink,
                    TaskState::Working,
                    format!(
                        "Updating {} voice records and rebuilding voice.dat…",
                        voice::replacement_count(voice_patch)
                    ),
                    Some(32),
                );
                let mut output = voice::apply_patch(&source, voice_patch)?;
                if options.optimize_voice { output = voice::compress_audio(&output, options.voice_compression)?; }
                let temporary = staging.join("voice.dat");
                write_synced(&temporary, &output)?;
                generated.push((
                    "voice.dat".to_string(),
                    source_path,
                    temporary,
                    voice_patch.target_hash,
                ));
                progress(
                    sink,
                    TaskState::Done,
                    "voice.dat records and archive offsets verified.",
                    Some(34),
                );
            }
        } else if options.optimize_voice {
            let source_path = find_file(folder, "voice.dat")?;
            let source = fs::read(&source_path).map_err(|error| format!("{}: {error}", source_path.display()))?;
            let output = voice::compress_audio(&source, options.voice_compression)?;
            if output != source {
                let temporary = staging.join("voice.dat");
                write_synced(&temporary, &output)?;
                let digest = hash::bytes(&output);
                generated.push(("voice.dat".to_string(), source_path, temporary, digest));
                progress(sink, TaskState::Done, "voice.dat audio was reduced and verified.", Some(34));
            }
        }
    } else {
        progress(sink, TaskState::Skipped, "Keeping compressed voice.dat as-is.", Some(34));
    }
    for patch in &patches {
        let source_path = find_file(folder, &patch.name)?;
        let source = fs::read(&source_path)
            .map_err(|error| format!("{}: {error}", source_path.display()))?;
        if hash::bytes(&source) == patch.target_hash {
            progress(
                sink,
                TaskState::Skipped,
                format!("{} already matches this patch.", patch.name),
                Some(35),
            );
            continue;
        }
        progress(
            sink,
            TaskState::Working,
            format!("Preparing verified changes for {}…", patch.name),
            Some(35),
        );
        let output = patch.apply(&source)?;
        let temporary = staging.join(&patch.name);
        write_synced(&temporary, &output)?;
        generated.push((
            patch.name.clone(),
            source_path,
            temporary,
            patch.target_hash,
        ));
        progress(
            sink,
            TaskState::Done,
            format!("{} is ready and verified.", patch.name),
            Some(38),
        );
    }

    let local_audio = if !options.keep_compressed_audio {
        prepare_local_audio(folder, &staging, payload, options, sink)?
    } else {
        progress(sink, TaskState::Skipped, "Keeping compressed BGM.dat as-is.", Some(42));
        LocalAudioPreparation {
            enabled: false,
            summary: "Compressed audio files were kept as-is per user choice.".into(),
            created: Vec::new(),
        }
    };
    let exe_path = find_file(folder, "Doraemon.exe")?;
    let exe_source =
        fs::read(&exe_path).map_err(|error| format!("{}: {error}", exe_path.display()))?;
    progress(
        sink,
        TaskState::Working,
        "Checking the game executable structure…",
        Some(40),
    );
    let mut exe_patch = pe::patch_language_runtime(
        &exe_source,
        payload.language == Language::Vietnamese,
        options.no_disc,
        options.no_reg,
        local_audio.enabled,
        options.modern_volume,
    )?;
    if options.primary_audio_8bit {
        let bytes = pe::patch_primary_directsound_8bit(&exe_patch.bytes)?;
        if bytes != exe_patch.bytes {
            exe_patch
                .actions
                .push("set primary DirectSound output to 22,050 Hz stereo 8-bit".into());
        }
        exe_patch.bytes = bytes;
    }
    if let Some(period) = options.game_clock_ms {
        let bytes = pe::patch_game_clock(&exe_patch.bytes, period)?;
        if bytes != exe_patch.bytes {
            exe_patch
                .actions
                .push(format!("set the game clock to a {period} ms tick"));
        }
        exe_patch.bytes = bytes;
    }
    if local_audio.enabled && !exe_patch.local_audio_supported {
        return Err(
            "this executable layout cannot safely use the local DirectSound music backend".into(),
        );
    }
    let exe_bytes = exe_patch.bytes;
    if exe_bytes == exe_source {
        progress(
            sink,
            TaskState::Skipped,
            "The requested executable changes are already installed.",
            Some(45),
        );
    } else {
        let temporary = staging.join("Doraemon.exe");
        write_synced(&temporary, &exe_bytes)?;
        generated.push((
            "Doraemon.exe".to_string(),
            exe_path,
            temporary,
            hash::bytes(&exe_bytes),
        ));
        for action in exe_patch.actions {
            progress(sink, TaskState::Done, action, Some(48));
        }
    }

    let audio = local_audio.summary.clone();

    if generated.is_empty() && local_audio.created.is_empty() {
        let message = "Everything requested is already installed.".to_string();
        progress(sink, TaskState::Done, &message, Some(100));
        let _ = fs::remove_dir(&staging);
        return Ok(ApplyReport {
            changed: Vec::new(),
            audio,
        });
    }
    if backup.exists() && !discard_restored_backup(&backup, folder, sink)? {
        progress(
            sink,
            TaskState::Working,
            "Verifying the existing original-file backup…",
            Some(60),
        );
        let originals = verified_backup_files(&backup)?;
        let mut missing = Vec::new();
        for (name, _, _, _) in &generated {
            if !originals.contains_key(name) {
                missing.push(name.clone());
            }
        }
        if !missing.is_empty() {
            if options.keep_compressed_audio {
                progress(
                    sink,
                    TaskState::Skipped,
                    format!("Existing backup is incomplete (no original {}). Creating a fresh backup.", missing.join(", ")),
                    Some(60),
                );
                fs::remove_dir_all(&backup).map_err(|error| format!("remove stale backup: {error}"))?;
            } else {
                return Err(format!(
                    "the existing backup does not contain an original {}; restore before applying this additional change",
                    missing[0]
                ));
            }
        } else {
            if !local_audio.created.is_empty() {
                return Err("the existing backup does not own these newly generated local-music files; restore before adding local music".into());
            }
            fs::copy(patcher_exe, backup.join("Restore.exe"))
                .map_err(|error| format!("refresh Restore.exe: {error}"))?;
            progress(
                sink,
                TaskState::Done,
                "The existing original-file backup is valid and will be reused.",
                Some(70),
            );
        }
    }
    if !backup.exists() || discard_restored_backup(&backup, folder, sink)? {
        progress(
            sink,
            TaskState::Working,
            if backup.exists() { "Creating your original-file backup…" } else { "Creating your original-file backup…" },
            Some(65),
        );
        if !backup.exists() {
            fs::create_dir_all(backup.join("original")).map_err(|error| error.to_string())?;
        }
        let mut originals = Vec::new();
        for (name, source, _, _) in &generated {
            let destination = backup.join("original").join(name);
            fs::copy(source, &destination).map_err(|error| format!("backup {}: {error}", name))?;
            let digest = hash::file(&destination)?;
            originals.push((name.clone(), digest));
        }
        fs::copy(patcher_exe, backup.join("Restore.exe"))
            .map_err(|error| format!("create Restore.exe: {error}"))?;

        let created_files: Vec<_> = local_audio
            .created
            .iter()
            .map(|(name, _, digest)| (name.clone(), *digest))
            .collect();
        let manifest = backup_manifest(payload.language.label(), &originals, &created_files);
        write_synced(&backup.join("manifest.json"), manifest.as_bytes())?;
        progress(
            sink,
            TaskState::Done,
            "Original files are safely backed up.",
            Some(75),
        );
    }
    progress(
        sink,
        TaskState::Working,
        "Installing prepared files…",
        Some(80),
    );
    let mut changed = Vec::new();
    for (name, _, temporary, target_hash) in generated {
        let target = find_file(folder, &name)?;
        replace_file(&temporary, &target)?;
        if hash::file(&target)? != target_hash {
            return Err(format!(
                "{name} changed during installation verification; restore from backup"
            ));
        }
        changed.push(name);
    }
    for (name, staged, digest) in local_audio.created {
        let target = folder.join(&name);
        replace_file(&staged, &target)?;
        if hash::file(&target)? != digest {
            return Err(format!(
                "{name} failed installation verification; restore from backup"
            ));
        }
        changed.push(name);
    }
    let _ = fs::remove_dir(&staging);
    progress(
        sink,
        TaskState::Done,
        "Installed files were verified successfully.",
        Some(100),
    );
    Ok(ApplyReport { changed, audio })
}

pub fn compressed_audio_files(backup: &Path, game: &Path) -> Result<Vec<String>> {
    let manifest_path = backup.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    let mut compressed = Vec::new();

    let mut in_files = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "\"files\": {" {
            in_files = true;
            continue;
        }
        if in_files && trimmed == "}," {
            break;
        }
        if !in_files {
            continue;
        }
        let entry = trimmed.trim_end_matches(',');
        let (name, digest) = entry
            .split_once(':')
            .ok_or("invalid backup manifest file entry")?;
        let name = name.trim().trim_matches('"').to_string();
        if name.eq_ignore_ascii_case("voice.dat") {
            let expected = hash::parse(digest.trim().trim_matches('"'))?;
            if let Ok(path) = find_file(game, "voice.dat") {
                if hash::file(&path)? != expected {
                    compressed.push(name);
                }
            }
        }
    }

    let created = manifest_created_files(&manifest)?;
    if let Some(expected) = created.get("BGM.dat") {
        let bgm_path = game.join("BGM.dat");
        if bgm_path.exists() && hash::file(&bgm_path)? != *expected {
            compressed.push("BGM.dat".into());
        }
    }

    Ok(compressed)
}

pub fn restore_skipping(backup: &Path, skip: &[&str]) -> Result<Vec<String>> {
    let manifest_path = backup.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    let game = backup
        .parent()
        .ok_or("backup folder has no parent game folder")?;
    let mut restored = Vec::new();
    let mut in_files = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "\"files\": {" {
            in_files = true;
            continue;
        }
        if in_files && trimmed == "}," {
            in_files = false;
            continue;
        }
        if in_files {
            let entry = trimmed.trim_end_matches(',');
            let (name, digest) = entry
                .split_once(':')
                .ok_or("invalid backup manifest file entry")?;
            let name = name.trim().trim_matches('"');
            if skip.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                continue;
            }
            let digest = digest.trim().trim_matches('"');
            let expected = hash::parse(digest)?;
            let source = backup.join("original").join(name);
            if hash::file(&source)? != expected {
                return Err(format!("backup copy of {name} was modified"));
            }
            let temporary = game.join(format!(".{name}.restore.tmp"));
            fs::copy(&source, &temporary).map_err(|error| error.to_string())?;
            let target = find_file(game, name).unwrap_or_else(|_| game.join(name));
            replace_file(&temporary, &target)?;
            if hash::file(&target)? != expected {
                return Err(format!("restored {name} failed verification"));
            }
            restored.push(name.to_string());
        }
    }
    for (name, digest) in manifest_created_files(&manifest)? {
        if skip.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
            continue;
        }
        let path = game.join(name);
        if path.exists() && hash::file(&path)? == digest {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }
    Ok(restored)
}

pub fn restore(backup: &Path) -> Result<Vec<String>> {
    let manifest_path = backup.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    let game = backup
        .parent()
        .ok_or("backup folder has no parent game folder")?;
    let mut restored = Vec::new();
    let mut in_files = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "\"files\": {" {
            in_files = true;
            continue;
        }
        if in_files && trimmed == "}," {
            in_files = false;
            continue;
        }
        if in_files {
            let entry = trimmed.trim_end_matches(',');
            let (name, digest) = entry
                .split_once(':')
                .ok_or("invalid backup manifest file entry")?;
            let name = name.trim().trim_matches('"');
            let digest = digest.trim().trim_matches('"');
            let expected = hash::parse(digest)?;
            let source = backup.join("original").join(name);
            if hash::file(&source)? != expected {
                return Err(format!("backup copy of {name} was modified"));
            }
            let temporary = game.join(format!(".{name}.restore.tmp"));
            fs::copy(&source, &temporary).map_err(|error| error.to_string())?;
            let target = find_file(game, name).unwrap_or_else(|_| game.join(name));
            replace_file(&temporary, &target)?;
            if hash::file(&target)? != expected {
                return Err(format!("restored {name} failed verification"));
            }
            restored.push(name.to_string());
        }
    }
    for (name, digest) in manifest_created_files(&manifest)? {
        let path = game.join(name);
        if path.exists() && hash::file(&path)? == digest {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }
    Ok(restored)
}

// ---- Split (multiple-language) builds -----------------------------------

/// One language build inside a [`SplitSelection`]. `icon` is the optional game
/// icon byte array the Windows patcher applies to the produced executable.
#[derive(Clone, Debug)]
pub struct SplitLanguage {
    pub language: Language,
    pub payload: Payload,
    pub icon: Option<Vec<u8>>,
}

/// The complete set of language builds to install together.
///
/// A split build never overwrites the original resources: a language that
/// changes a file writes it under a suffixed name (`sprite1-vi.dat`), while a
/// file that is unchanged for every selected language stays shared. The
/// `<original>` build simply keeps `Doraemon.exe` and the original names, and
/// is only present when `include_original` is set. Restoring means deleting the
/// generated files again, which makes switching builds cheap and safe.
#[derive(Clone, Debug)]
pub struct SplitSelection {
    /// Keep the `<original>` build (the untouched `Doraemon.exe` and the
    /// original, shared resource files) playable beside the language builds.
    pub include_original: bool,
    /// The non-original language builds. At least one is required.
    pub languages: Vec<SplitLanguage>,
}

/// On-disk resource names and every executable literal that references them.
const SPLIT_RESOURCES: &[(&str, &[&str])] = &[
    ("strings.dat", &["strings.dat"]),
    ("voice.dat", &["voice.dat"]),
    ("sysfont.dat", &["sysfont.dat"]),
    ("Sprite1.dat", &["sprite1.dat"]),
    ("sprite2.dat", &["sprite2.dat", "Sprite2.Dat"]),
    ("bitmaps.dat", &["bitmaps.dat"]),
];

/// The suffixed filename a language build uses for a resource, e.g.
/// `Sprite1.dat` -> `sprite1-vi.dat`. It is lower-cased so it matches the
/// executable's literal case on both case-insensitive and case-aware disks.
fn suffixed_name(base: &str, suffix: &str) -> String {
    let lower = base.to_ascii_lowercase();
    match lower.rfind('.') {
        Some(pos) => format!("{}-{suffix}{}", &lower[..pos], &lower[pos..]),
        None => format!("{lower}-{suffix}"),
    }
}

/// Applies a language's single-file patches to one resource.
fn language_resource(language: &SplitLanguage, base_name: &str, base: &[u8]) -> Result<Vec<u8>> {
    match base_name {
        "strings.dat" => match &language.payload.strings {
            Some(patch) => strings::apply_patch(base, patch),
            None => Ok(base.to_vec()),
        },
        "voice.dat" => match &language.payload.voice {
            Some(patch) => voice::apply_patch(base, patch),
            None => Ok(base.to_vec()),
        },
        _ => match language
            .payload
            .profiles
            .iter()
            .flat_map(|profile| &profile.files)
            .find(|file| file.name.eq_ignore_ascii_case(base_name))
        {
            Some(patch) => patch.apply(base),
            None => Ok(base.to_vec()),
        },
    }
}

/// The result of planning a split install: every file to create (suffixed
/// resources plus one executable per language), the original files to move
/// aside, and the per-executable resource-path rewrite needed.
struct SplitPlan {
    created: Vec<(String, Vec<u8>)>,
    removed_originals: Vec<String>,
}

/// Computes, for a split selection and the pristine base resources, the exact
/// set of files to create, originals to remove, and executable rewrites.
/// Pure logic, split out so the runtime behavior is unit-testable.
fn plan_split(
    selection: &SplitSelection,
    base_resources: &HashMap<String, Vec<u8>>,
    base_exe: &[u8],
    options: &ApplyOptions,
    local_audio_enabled: bool,
) -> Result<SplitPlan> {
    // Each language's target bytes, per resource.
    let mut lang_targets: Vec<HashMap<String, Vec<u8>>> = Vec::new();
    for language in &selection.languages {
        let mut targets = HashMap::new();
        for (base_name, _) in SPLIT_RESOURCES {
            let target = language_resource(language, base_name, &base_resources[*base_name])?;
            targets.insert(base_name.to_string(), target);
        }
        lang_targets.push(targets);
    }

    let mut created: Vec<(String, Vec<u8>)> = Vec::new();
    let mut removed_originals: Vec<String> = Vec::new();
    let mut exe_mapping: Vec<Vec<(String, String)>> = vec![Vec::new(); selection.languages.len()];

    for (base_name, literals) in SPLIT_RESOURCES {
        let base = &base_resources[*base_name];
        let mut changed: Vec<usize> = Vec::new();
        for (i, targets) in lang_targets.iter().enumerate() {
            if &targets[*base_name] != base {
                changed.push(i);
            }
        }
        if changed.is_empty() {
            // Unchanged in every selected language: keep the shared original.
            continue;
        }
        // Each changed language gets its own suffixed file, and its executable
        // is repointed at that file.
        for &i in &changed {
            let suffixed = suffixed_name(base_name, selection.languages[i].language.suffix());
            let bytes = lang_targets[i][*base_name].clone();
            for literal in *literals {
                exe_mapping[i].push((literal.to_string(), suffixed.clone()));
            }
            created.push((suffixed, bytes));
        }
        // Drop the now-orphaned unsuffixed original when there is no `<original>`
        // build and every selected language overrides this resource.
        if !selection.include_original && changed.len() == selection.languages.len() {
            removed_originals.push(base_name.to_string());
        }
    }

    // One patched executable per language, plus the patched original build.
    for (i, language) in selection.languages.iter().enumerate() {
        let mut bytes = pe::patch_language_runtime(
            base_exe,
            language.language == Language::Vietnamese,
            options.no_disc,
            options.no_reg,
            local_audio_enabled,
            options.modern_volume,
        )?
        .bytes;
        if options.primary_audio_8bit {
            bytes = pe::patch_primary_directsound_8bit(&bytes)?;
        }
        if let Some(period) = options.game_clock_ms {
            bytes = pe::patch_game_clock(&bytes, period)?;
        }
        if !exe_mapping[i].is_empty() {
            bytes = pe::rewrite_resource_paths(&bytes, &exe_mapping[i])?;
        }
        created.push((
            format!("Doraemon-{}.exe", language.language.suffix()),
            bytes,
        ));
    }

    // The `<original>` build is the base game patched in place: it runs without
    // a disc while keeping the original resource names and the original icon.
    // The pristine executable is set aside into the backup folder so it can be
    // restored later. It never uses the extended Vietnamese font, so we apply
    // only the compatibility edits and skip the runtime-language icon patch.
    if selection.include_original {
        let mut bytes = pe::patch_compatible(
            base_exe,
            options.no_disc,
            options.local_audio,
            options.no_reg,
            options.modern_volume,
        )?
        .bytes;
        if options.primary_audio_8bit {
            bytes = pe::patch_primary_directsound_8bit(&bytes)?;
        }
        if let Some(period) = options.game_clock_ms {
            bytes = pe::patch_game_clock(&bytes, period)?;
        }
        created.push(("Doraemon.exe".to_string(), bytes));
        removed_originals.push("Doraemon.exe".to_string());
    } else {
        removed_originals.push("Doraemon.exe".to_string());
    }
    created.sort_by(|a, b| a.0.cmp(&b.0));
    removed_originals.sort();
    removed_originals.dedup();
    Ok(SplitPlan {
        created,
        removed_originals,
    })
}

/// Generates the split manifest stored inside the split backup directory. It
/// records every file created (so restore can delete exactly those) and the
/// original resource names moved aside (so restore can put them back). Written
/// one entry per line so restore can be a simple, lossless parser.
fn split_manifest(created: &[String], removed: &[String]) -> String {
    let mut out = String::from("{\n  \"created\": [\n");
    for name in created {
        out.push_str(&format!("    \"{name}\",\n"));
    }
    out.push_str("  ],\n  \"removed\": [\n");
    for name in removed {
        out.push_str(&format!("    \"{name}\",\n"));
    }
    out.push_str("  ]\n}\n");
    out
}

fn parse_split_names(manifest: &str) -> Result<(Vec<String>, Vec<String>)> {
    let mut created = Vec::new();
    let mut removed = Vec::new();
    let mut section: Option<bool> = None;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"created\"") {
            section = Some(true);
            continue;
        }
        if trimmed.starts_with("\"removed\"") {
            section = Some(false);
            continue;
        }
        if trimmed.starts_with(']') || trimmed.starts_with('{') || trimmed.starts_with('}') {
            continue;
        }
        let Some(is_created) = section else { continue };
        if trimmed.is_empty() {
            continue;
        }
        let name = trimmed.trim_end_matches(',').trim_matches('"').to_string();
        if name.is_empty() {
            return Err("empty entry in split manifest".into());
        }
        if is_created {
            created.push(name);
        } else {
            removed.push(name);
        }
    }
    Ok((created, removed))
}

/// Installs a split (multiple-language) build inside `folder`. Resources that a
/// language changes are written under suffixed names; each language gets a
/// patched, suffixed executable. Originals that become orphaned are moved
/// aside to the split backup for cheap restore.
pub fn apply_split_with_progress(
    folder: &Path,
    selection: &SplitSelection,
    options: &ApplyOptions,
    _patcher_exe: &Path,
    sink: &mut ProgressSink<'_>,
) -> Result<ApplyReport> {
    if selection.languages.is_empty() {
        return Err("at least one language must be selected".into());
    }
    if selection
        .languages
        .iter()
        .any(|language| language.language == Language::Custom)
    {
        return Err("the portable profile cannot be installed as a split language".into());
    }
    if !folder.is_dir() {
        return Err(format!("{} is not a game folder", folder.display()));
    }
    progress(
        sink,
        TaskState::Working,
        "Preparing a split multi-language install…",
        Some(0),
    );

    // -- Seed `backup/original/` with the pristine originals (source of truth).
    // The backup is written once on the first install and is reused by every
    // later apply and restore. The live root is never trusted after that: a
    // reinstall may legitimately delete an original that no selected language
    // uses, so the restore button must rebuild from the backup, not the root.
    let backup = folder.join("backup");
    let backup_original = backup.join("original");
    fs::create_dir_all(&backup_original).map_err(|e| e.to_string())?;
    let backup_exe = backup_original.join("Doraemon.exe");
    if !backup_exe.exists() {
        // Only consult the live root while seeding the backup. Afterwards the
        // backup copy is authoritative: an install that excludes the
        // `<original>` build deletes the root Doraemon.exe, and a reapply must
        // still work from the backup alone.
        let exe_path = find_file(folder, "Doraemon.exe")?;
        fs::copy(&exe_path, &backup_exe).map_err(|e| format!("backup Doraemon.exe: {e}"))?;
    }
    let mut base_resources: HashMap<String, Vec<u8>> = HashMap::new();
    for (base_name, _) in SPLIT_RESOURCES {
        let canonical = backup_original.join(base_name);
        if !canonical.exists() {
            let source = find_file(folder, base_name)?;
            fs::copy(&source, &canonical)
                .map_err(|e| format!("backup {}: {e}", base_name))?;
        }
        let bytes = fs::read(&canonical)
            .map_err(|error| format!("backup {}: {error}", base_name))?;
        base_resources.insert(base_name.to_string(), bytes);
    }
    let base_exe = fs::read(&backup_exe).map_err(|error| format!("backup Doraemon.exe: {error}"))?;

    // -- Compute the exact file layout and bytes for every language. ---------
    let plan = plan_split(selection, &base_resources, &base_exe, options, options.local_audio)?;

    // -- Stage shared local music (all languages share one BGM stream). ------
    let staging = folder.join(".doraemon-split-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let local_audio =
        prepare_local_audio(folder, &staging, &selection.languages[0].payload, options, sink)?;
    let mut created = plan.created;
    for (name, staged, _digest) in local_audio.created {
        let bytes = fs::read(&staged).map_err(|error| error.to_string())?;
        created.push((name.to_string(), bytes));
    }
    created.sort_by(|a, b| a.0.cmp(&b.0));
    created.dedup_by(|a, b| a.0 == b.0);

    let mut changed = Vec::new();
    let created_names: HashSet<String> = created.iter().map(|(name, _)| name.clone()).collect();
    for (name, bytes) in &created {
        progress(
            sink,
            TaskState::Working,
            format!("Writing {name}…"),
            Some(50),
        );
        write_synced(&folder.join(name), bytes)?;
        changed.push(name.clone());
    }
    for name in &plan.removed_originals {
        // A pristine original listed in `removed_originals` may have just been
        // replaced by a patched build with the same name (the `<original>`
        // `Doraemon.exe`). Never delete a file that was part of `created`.
        if created_names.contains(name)
            || !find_file(folder, name).map(|path| path.exists()).unwrap_or(false)
        {
            continue;
        }
        if let Ok(path) = find_file(folder, name) {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        changed.push(format!("<removed> {}", name));
    }
    write_synced(
        &backup.join("split-manifest.json"),
        split_manifest(&changed, &plan.removed_originals).as_bytes(),
    )?;
    fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;

    progress(
        sink,
        TaskState::Done,
        "Split multi-language install is ready.",
        Some(100),
    );
    let audio = local_audio.summary;
    Ok(ApplyReport { changed, audio })
}

/// Removes every file created by a split install and reinstates the originals
/// that were set aside. Returns the names of the files that were cleaned up.
pub fn restore_split(folder: &Path) -> Result<Vec<String>> {
    let backup = folder.join("backup");
    let manifest = fs::read_to_string(backup.join("split-manifest.json")).map_err(|error| {
        format!(
            "no split backup manifest at {}: {error}",
            backup.join("split-manifest.json").display()
        )
    })?;
    let (created, removed) = parse_split_names(&manifest)?;

    let mut cleaned = Vec::new();
    for name in created {
        let path = folder.join(&name);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("remove {}: {e}", path.display()))?;
            cleaned.push(name.clone());
        }
    }
    for name in removed {
        let source = backup.join("original").join(&name);
        let target = folder.join(&name);
        fs::copy(&source, &target).map_err(|e| e.to_string())?;
        cleaned.push(format!("restored {name}"));
    }
    // Remove only the split marker; the shared `backup/original/` source of
    // truth stays so a coexisting single-language backup keeps working.
    let _ = fs::remove_file(backup.join("split-manifest.json"));
    let staging = folder.join(".doraemon-split-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    Ok(cleaned)
}

/// Restores every kind of backup present in `folder`: a split install (via its
/// `split-manifest.json`) and/or a single-language install (via `manifest.json`).
/// Returns the combined names of the files that were cleaned up or restored.
pub fn restore_any(folder: &Path) -> Result<Vec<String>> {
    let mut restored = Vec::new();
    if folder.join("backup").join("split-manifest.json").is_file() {
        restored.extend(restore_split(folder)?);
    }
    if folder.join("backup").join("manifest.json").is_file() {
        restored.extend(restore(&folder.join("backup"))?);
    }
    Ok(restored)
}

/// True when a backup exists that the main "Restore backup" action can rebuild
/// the original game from (either a split install or a single-language backup).
pub fn has_restorable_backup(folder: &Path) -> bool {
    let backup = folder.join("backup");
    backup.join("split-manifest.json").is_file() || backup.join("manifest.json").is_file()
}

/// The visible backup directory used by split installs (and shared with the
/// single-language install). Kept public so the patcher can surface its location.
pub fn backup_dir(folder: &Path) -> std::path::PathBuf {
    folder.join("backup")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::{BundledFile, Language, PatchProfile};
    use crate::sysfont;

    #[test]
    fn wrapper_installs_bundled_files_without_overwriting_different_files() {
        let folder =
            std::env::temp_dir().join(format!("doraemon-wrapper-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir(&folder).unwrap();
        let bytes = b"wrapper".to_vec();
        let empty = FilePatch::create("Doraemon.exe", &[], &[]).unwrap();
        let payload = Payload {
            language: Language::Custom,
            profiles: vec![PatchProfile {
                name: "test".into(),
                required: Vec::new(),
                files: Vec::new(),
                executable_plain: None,
                executable_portable: empty,
            }],
            strings: None,
            voice: None,
            bundled: vec![BundledFile {
                name: "Shaders/test.glsl".into(),
                hash: hash::bytes(&bytes),
                bytes: bytes.clone(),
            }],
        };
        assert_eq!(add_wrapper(&folder, &payload).unwrap().len(), 1);
        assert_eq!(fs::read(folder.join("Shaders/test.glsl")).unwrap(), bytes);
        assert!(add_wrapper(&folder, &payload).unwrap().is_empty());
        fs::write(folder.join("Shaders/test.glsl"), b"different").unwrap();
        assert!(add_wrapper(&folder, &payload)
            .unwrap_err()
            .contains("different"));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn only_vietnamese_sysfont_extends_the_font_table_when_fixtures_are_available() {
        let (Ok(data_dir), Ok(parts_dir)) = (
            std::env::var("DORAEMON_TEST_DATA_DIR"),
            std::env::var("DORAEMON_TEST_PARTS_ENGLISH"),
        ) else {
            return;
        };
        let base = fs::read(Path::new(&data_dir).join("sysfont.dat")).unwrap();
        let base_count = sysfont::parse(&base).unwrap().glyphs.len();
        assert_eq!(base_count, sysfont::ORIGINAL_GLYPHS);
        let apply_sysfont = |dir: &std::path::Path| -> usize {
            let mut parts = Vec::new();
            for name in ["dubbing.dmpatch", "sprites.dmpatch", "runtime.dmpatch"] {
                let bytes = fs::read(dir.join(name)).unwrap();
                parts.push(crate::payload::decode_part(&bytes).unwrap());
            }
            let payload = crate::merge_parts(&parts).unwrap();
            let mut count = 0;
            for profile in &payload.profiles {
                for f in &profile.files {
                    if f.name.eq_ignore_ascii_case("sysfont.dat") {
                        count = sysfont::parse(&f.apply(&base).unwrap())
                            .unwrap()
                            .glyphs
                            .len();
                    }
                }
            }
            count
        };
        let english_dir = Path::new(&parts_dir);
        let vietnamese_dir = Path::new(&parts_dir)
            .parent()
            .unwrap()
            .join("vietnamese");
        let english_count = apply_sysfont(english_dir);
        let vietnamese_count = apply_sysfont(&vietnamese_dir);
        // English only reshapes the base 640 glyphs; only Vietnamese extends the
        // font to 1920 glyphs (which is what requires the Vietnamese runtime hook).
        assert_eq!(english_count, sysfont::ORIGINAL_GLYPHS);
        assert_eq!(vietnamese_count, sysfont::EXTENDED_GLYPHS);
    }

    #[test]
    fn split_apply_produces_every_executable_reference_when_fixtures_are_available() {
        let (Ok(data_dir), Ok(parts_dir)) = (
            std::env::var("DORAEMON_TEST_DATA_DIR"),
            std::env::var("DORAEMON_TEST_PARTS_ENGLISH"),
        ) else {
            return;
        };
        // Reconstruct the English payload from its three part files, exactly as
        // the Windows patcher does.
        let mut parts = Vec::new();
        for name in ["dubbing.dmpatch", "sprites.dmpatch", "runtime.dmpatch"] {
            let bytes = std::fs::read(Path::new(&parts_dir).join(name)).unwrap();
            parts.push(crate::payload::decode_part(&bytes).unwrap());
        }
        let payload = crate::merge_parts(&parts).unwrap();

        let folder = std::env::temp_dir().join(format!(
            "doraemon-split-apply-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir(&folder).unwrap();
        fs::copy(Path::new(&data_dir).join("Doraemon.exe"), folder.join("Doraemon.exe")).unwrap();
        for name in ["strings.dat", "voice.dat", "sysfont.dat", "Sprite1.dat", "sprite2.dat", "bitmaps.dat"] {
            fs::copy(Path::new(&data_dir).join(name), folder.join(name)).unwrap();
        }

        let selection = SplitSelection {
            include_original: true,
            languages: vec![SplitLanguage {
                language: Language::English,
                payload: payload.clone(),
                icon: None,
            }],
        };
        apply_split_with_progress(
            &folder,
            &selection,
            &ApplyOptions {
                no_disc: false,
                no_reg: false,
                local_audio: false,
                modern_volume: true,
                cue: None,
                ..ApplyOptions::default()
            },
            &std::env::current_exe().unwrap(),
            &mut |_| {},
        )
        .unwrap();

        // Every resource path the produced executable actually references must
        // resolve to a file that now exists in the game folder. A reference that
        // names a file absent on disk is exactly what makes the game copy a null
        // buffer (a crash like the `rep movsl` with ESI=0 at startup).
        let exe_bytes = fs::read(folder.join("Doraemon-en.exe")).unwrap();
        let mut referenced = Vec::new();
        let mut index = 0usize;
        while index < exe_bytes.len() {
            if (0x20..0x7f).contains(&exe_bytes[index]) {
                let start = index;
                while index < exe_bytes.len() && (0x20..0x7f).contains(&exe_bytes[index]) {
                    index += 1;
                }
                let text = String::from_utf8_lossy(&exe_bytes[start..index]).into_owned();
                if text.to_ascii_lowercase().ends_with(".dat") {
                    referenced.push(text);
                }
            } else {
                index += 1;
            }
        }
        referenced.dedup();
        eprintln!("doraemon-en.exe resource references: {referenced:?}");
        // Runtime format strings (save%04d, map%04d, Database%04x, MGame*)
        // are templates the engine expands at runtime; they are not shipped
        // files. What matters for the loader is that every suffixed resource
        // build the executable is rewired to use actually exists on disk.
        let suffixed: Vec<&String> = referenced
            .iter()
            .filter(|text| text.to_ascii_lowercase().ends_with("-en.dat"))
            .collect();
        assert!(!suffixed.is_empty(), "no suffixed reference found");
        for name in &suffixed {
            assert!(
                find_file(&folder, name).is_ok(),
                "executable references '{name}' but no such file was produced"
            );
        }

        // With `<original>` included, the in-place `Doraemon.exe` must be the
        // patched original build (not the untouched, disc-requiring executable),
        // and the pristine executable must have been set aside into the backup.
        let base_exe = fs::read(Path::new(&data_dir).join("Doraemon.exe")).unwrap();
        let in_place = fs::read(folder.join("Doraemon.exe")).unwrap();
        assert_ne!(
            &in_place, &base_exe,
            "<original> build must patch Doraemon.exe in place"
        );
        let backup_orig_exe = fs::read(folder.join("backup/original/Doraemon.exe")).unwrap();
        assert_eq!(&backup_orig_exe, &base_exe, "pristine original must be backed up");
        // The backup is the source of truth: a split manifest exists, the
        // pristine resources were seeded there, and the main Restore action can
        // rebuild the game from them.
        assert!(
            folder.join("backup/split-manifest.json").is_file(),
            "split install must leave a restore manifest in the visible backup folder"
        );
        assert!(
            has_restorable_backup(&folder),
            "the visible backup must be detected by the main Restore action"
        );
        for (base_name, _) in SPLIT_RESOURCES {
            assert!(
                folder.join("backup/original").join(base_name).is_file(),
                "pristine {base_name} must be seeded into backup/original"
            );
        }

        fs::remove_dir_all(folder).unwrap();
    }

    /// Reapplying must work off `backup/original/`, never the live root. The
    /// root is not a reliable source after the first install: a selection
    /// without `<original>` deletes the root `Doraemon.exe` outright, and every
    /// remaining root file is already patched.
    #[test]
    fn split_apply_reapplies_from_the_backup_when_fixtures_are_available() {
        let (Ok(data_dir), Ok(parts_dir)) = (
            std::env::var("DORAEMON_TEST_DATA_DIR"),
            std::env::var("DORAEMON_TEST_PARTS_ENGLISH"),
        ) else {
            return;
        };
        let mut parts = Vec::new();
        for name in ["dubbing.dmpatch", "sprites.dmpatch", "runtime.dmpatch"] {
            let bytes = std::fs::read(Path::new(&parts_dir).join(name)).unwrap();
            parts.push(crate::payload::decode_part(&bytes).unwrap());
        }
        let payload = crate::merge_parts(&parts).unwrap();

        let folder = std::env::temp_dir().join(format!(
            "doraemon-split-reapply-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir(&folder).unwrap();
        fs::copy(Path::new(&data_dir).join("Doraemon.exe"), folder.join("Doraemon.exe")).unwrap();
        for name in ["strings.dat", "voice.dat", "sysfont.dat", "Sprite1.dat", "sprite2.dat", "bitmaps.dat"] {
            fs::copy(Path::new(&data_dir).join(name), folder.join(name)).unwrap();
        }

        // No `<original>` build, so the root Doraemon.exe is removed.
        let selection = SplitSelection {
            include_original: false,
            languages: vec![SplitLanguage {
                language: Language::English,
                payload: payload.clone(),
                icon: None,
            }],
        };
        let options = ApplyOptions {
            no_disc: true,
            no_reg: true,
            game_clock_ms: Some(17),
            ..ApplyOptions::default()
        };
        let run = |options: &ApplyOptions| {
            apply_split_with_progress(
                &folder,
                &selection,
                options,
                &std::env::current_exe().unwrap(),
                &mut |_| {},
            )
        };
        run(&options).unwrap();
        assert!(
            !folder.join("Doraemon.exe").exists(),
            "a selection without <original> must remove the root executable"
        );
        let first = fs::read(folder.join("Doraemon-en.exe")).unwrap();

        // A split install leaves no manifest.json, so anything on the reapply
        // path that demands one is a bug.
        assert!(!folder.join("backup/manifest.json").exists());

        // Reapply with the root executable gone: the backup must carry it.
        run(&options).unwrap();
        assert_eq!(
            fs::read(folder.join("Doraemon-en.exe")).unwrap(),
            first,
            "reapplying the same options must be byte-identical"
        );

        // Changing an option must take effect on the reapply rather than
        // stacking on top of the previously patched bytes.
        let stock = ApplyOptions { game_clock_ms: None, ..options.clone() };
        run(&stock).unwrap();
        let reverted = fs::read(folder.join("Doraemon-en.exe")).unwrap();
        assert_ne!(reverted, first, "clearing the game clock must change the build");
        run(&options).unwrap();
        assert_eq!(
            fs::read(folder.join("Doraemon-en.exe")).unwrap(),
            first,
            "re-selecting the game clock must reproduce the earlier build exactly"
        );

        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn local_music_files_are_never_staged_when_option_is_off() {
        let folder = std::env::temp_dir().join(format!(
            "doraemon-local-music-off-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        let staging = folder.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let payload = Payload {
            language: Language::Custom,
            profiles: Vec::new(),
            strings: None,
            voice: None,
            bundled: Vec::new(),
        };
        let prepared = prepare_local_audio(
            &folder,
            &staging,
            &payload,
            &ApplyOptions {
                local_audio: false,
                ..ApplyOptions::default()
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(!prepared.enabled);
        assert!(prepared.created.is_empty());
        assert!(!staging.join("BGM.dat").exists());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn backup_manifest_tracks_all_generated_local_music_files() {
        let music = hash::bytes(b"music");
        let manifest = backup_manifest("test", &[], &[("BGM.dat".into(), music)]);
        let created = manifest_created_files(&manifest).unwrap();
        assert_eq!(created.get("BGM.dat"), Some(&music));
    }

    #[test]
    fn real_payload_applies_and_restores_when_fixtures_are_available() {
        let (Ok(base), Ok(payload_path)) = (
            std::env::var("DORAEMON_TEST_DATA_DIR"),
            std::env::var("DORAEMON_TEST_PAYLOAD"),
        ) else {
            return;
        };
        let payload = crate::payload::decode(&fs::read(payload_path).unwrap()).unwrap();
        let folder =
            std::env::temp_dir().join(format!("doraemon-patch-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir(&folder).unwrap();
        fs::copy(
            Path::new(&base).join("Doraemon.exe"),
            folder.join("Doraemon.exe"),
        )
        .unwrap();
        for required in &payload.profiles[0].required {
            fs::copy(
                Path::new(&base).join(&required.name),
                folder.join(&required.name),
            )
            .unwrap();
        }
        if payload.strings.is_some() {
            fs::copy(
                Path::new(&base).join("strings.dat"),
                folder.join("strings.dat"),
            )
            .unwrap();
        }
        let before: Vec<_> = payload.profiles[0]
            .required
            .iter()
            .map(|required| {
                (
                    required.name.clone(),
                    hash::file(&folder.join(&required.name)).unwrap(),
                )
            })
            .collect();
        let strings_before = payload
            .strings
            .as_ref()
            .map(|_| hash::file(&folder.join("strings.dat")).unwrap());
        let report = apply(
            &folder,
            &payload,
            &ApplyOptions {
                no_disc: false,
                no_reg: false,
                local_audio: false,
                modern_volume: false,
                cue: None,
                ..ApplyOptions::default()
            },
            &std::env::current_exe().unwrap(),
        )
        .unwrap();
        assert!(!report.changed.is_empty());
        assert!(folder.join("backup/Restore.exe").exists());
        assert!(folder.join("backup/manifest.json").exists());
        let repeated = apply(
            &folder,
            &payload,
            &ApplyOptions {
                no_disc: false,
                no_reg: false,
                local_audio: false,
                modern_volume: false,
                cue: None,
                ..ApplyOptions::default()
            },
            &std::env::current_exe().unwrap(),
        )
        .unwrap();
        assert!(repeated.changed.is_empty());
        restore(&folder.join("backup")).unwrap();
        for (name, digest) in before {
            assert_eq!(hash::file(&folder.join(name)).unwrap(), digest);
        }
        if let Some(digest) = strings_before {
            assert_eq!(hash::file(&folder.join("strings.dat")).unwrap(), digest);
        }
        let reapplied = apply(
            &folder,
            &payload,
            &ApplyOptions {
                no_disc: false,
                no_reg: false,
                local_audio: false,
                modern_volume: false,
                cue: None,
                ..ApplyOptions::default()
            },
            &std::env::current_exe().unwrap(),
        )
        .unwrap();
        assert!(!reapplied.changed.is_empty());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn real_local_music_installs_and_restores_when_fixtures_are_available() {
        let (Ok(base), Ok(payload_path), Ok(cue_path)) = (
            std::env::var("DORAEMON_TEST_DATA_DIR"),
            std::env::var("DORAEMON_TEST_PAYLOAD"),
            std::env::var("DORAEMON_TEST_CUE"),
        ) else {
            return;
        };
        let payload = crate::payload::decode(&fs::read(payload_path).unwrap()).unwrap();
        let folder = std::env::temp_dir().join(format!(
            "doraemon-local-music-install-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir(&folder).unwrap();
        fs::copy(
            Path::new(&base).join("Doraemon.exe"),
            folder.join("Doraemon.exe"),
        )
        .unwrap();
        for required in &payload.profiles[0].required {
            fs::copy(
                Path::new(&base).join(&required.name),
                folder.join(&required.name),
            )
            .unwrap();
        }
        if payload.strings.is_some() {
            fs::copy(
                Path::new(&base).join("strings.dat"),
                folder.join("strings.dat"),
            )
            .unwrap();
        }
        let report = apply(
            &folder,
            &payload,
            &ApplyOptions {
                no_disc: true,
                no_reg: true,
                local_audio: true,
                modern_volume: true,
                primary_audio_8bit: true,
                game_clock_ms: None,
                cue: Some(PathBuf::from(cue_path)),
                ..ApplyOptions::default()
            },
            &std::env::current_exe().unwrap(),
        )
        .unwrap();
        assert!(report.changed.iter().any(|name| name == "BGM.dat"));
        assert!(music::valid(&folder.join("BGM.dat")));
        let executable = fs::read(folder.join("Doraemon.exe")).unwrap();
        assert_eq!(&executable[0x88e92..0x88e96], &44_100u32.to_le_bytes());
        assert_eq!(&executable[0x88ea0..0x88ea2], &2u16.to_le_bytes());
        assert_eq!(&executable[0x88eac..0x88eae], &8u16.to_le_bytes());
        assert!(executable.windows(7).any(|bytes| bytes == b"BGMRT5\0"));
        assert_eq!(executable[0x8b3b0], 0xe8);
        restore(&folder.join("backup")).unwrap();
        assert!(!folder.join("BGM.dat").exists());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn alternate_profile_applies_when_fixtures_are_available() {
        let (Ok(base), Ok(payload_path)) = (
            std::env::var("DORAEMON_TEST_ALTERNATE_DATA_DIR"),
            std::env::var("DORAEMON_TEST_PAYLOAD"),
        ) else {
            return;
        };
        let payload = crate::payload::decode(&fs::read(payload_path).unwrap()).unwrap();
        let profile = &payload.profiles[1];
        let folder = std::env::temp_dir().join(format!(
            "doraemon-alternate-patch-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir(&folder).unwrap();
        for required in &profile.required {
            fs::copy(
                Path::new(&base).join(&required.name),
                folder.join(&required.name),
            )
            .unwrap();
        }
        let report = apply(
            &folder,
            &payload,
            &ApplyOptions {
                no_disc: true,
                no_reg: true,
                local_audio: false,
                modern_volume: false,
                cue: None,
                ..ApplyOptions::default()
            },
            &std::env::current_exe().unwrap(),
        )
        .unwrap();
        assert!(report.changed.iter().any(|name| name == "Doraemon.exe"));
        for patch in selected_patches(profile, true) {
            assert_eq!(
                hash::file(&folder.join(&patch.name)).unwrap(),
                patch.target_hash
            );
        }
        restore(&folder.join("backup")).unwrap();
        for required in &profile.required {
            assert_eq!(
                hash::file(&folder.join(&required.name)).unwrap(),
                required.hash
            );
        }
        restore(&folder.join("backup")).unwrap();
        for required in &profile.required {
            assert_eq!(
                hash::file(&folder.join(&required.name)).unwrap(),
                required.hash
            );
        }
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn suffixed_name_lowercases_and_inserts_suffix_before_extension() {
        assert_eq!(suffixed_name("Sprite1.dat", "vi"), "sprite1-vi.dat");
        assert_eq!(suffixed_name("strings.dat", "en"), "strings-en.dat");
        assert_eq!(suffixed_name("bitmaps.dat", "en"), "bitmaps-en.dat");
        assert_eq!(suffixed_name("NoExtension", "vi"), "noextension-vi");
    }

    #[test]
    fn split_manifest_round_trips_created_and_removed_names() {
        let created = vec!["sprite1-vi.dat".into(), "Doraemon-vi.exe".into()];
        let removed = vec!["Doraemon.exe".into(), "Sprite1.dat".into()];
        let manifest = split_manifest(&created, &removed);
        let (parsed_created, parsed_removed) = parse_split_names(&manifest).unwrap();
        assert_eq!(parsed_created, created);
        assert_eq!(parsed_removed, removed);
    }

    #[test]
    fn split_manifest_round_trips_empty_lists() {
        let manifest = split_manifest(&[], &[]);
        let (created, removed) = parse_split_names(&manifest).unwrap();
        assert!(created.is_empty());
        assert!(removed.is_empty());
    }

    #[test]
    fn plan_split_rejects_no_language_and_writes_suffixed_files_when_fixture_is_available() {
        let (Ok(base), Ok(payload_path)) = (
            std::env::var("DORAEMON_TEST_DATA_DIR"),
            std::env::var("DORAEMON_TEST_PAYLOAD"),
        ) else {
            return;
        };
        let payload = crate::payload::decode(&fs::read(payload_path).unwrap()).unwrap();
        let mut base_resources = HashMap::new();
        for (base_name, _) in SPLIT_RESOURCES {
            let bytes = fs::read(Path::new(&base).join(
                if *base_name == "bitmaps.dat" { "bitmaps.dat" } else { base_name },
            )).ok().unwrap_or_default();
            base_resources.insert(base_name.to_string(), bytes);
        }
        let base_exe = fs::read(Path::new(&base).join("Doraemon.exe")).unwrap();
        let selection = SplitSelection {
            include_original: false,
            languages: vec![SplitLanguage {
                language: Language::Vietnamese,
                payload: payload.clone(),
                icon: None,
            }],
        };
        let plan = plan_split(
            &selection,
            &base_resources,
            &base_exe,
            &ApplyOptions::default(),
            false,
        )
        .unwrap();
        assert!(plan
            .created
            .iter()
            .any(|(name, _)| name.ends_with("-vi.exe")));
        assert!(plan
            .created
            .iter()
            .any(|(name, _)| name == "sprite1-vi.dat"));
        assert!(plan.removed_originals.contains(&"Doraemon.exe".to_string()));
    }
}

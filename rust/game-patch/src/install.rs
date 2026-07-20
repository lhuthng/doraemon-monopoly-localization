use std::{
    collections::HashMap,
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
    pub cue: Option<PathBuf>,
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

fn bundled_audio_helper(payload: &Payload) -> Option<&crate::payload::BundledFile> {
    payload
        .bundled
        .iter()
        .find(|file| file.name.eq_ignore_ascii_case("doraudio.dll"))
}

struct LocalAudioPreparation {
    enabled: bool,
    summary: String,
    created: Vec<(String, PathBuf, hash::Hash)>,
}

fn prepare_local_audio(
    folder: &Path,
    staging: &Path,
    payload: &Payload,
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
    let music_path = folder.join("Music.dat");
    let wav_path = folder.join("DoraemonMusic.wav");
    let source = if music::valid(&music_path) {
        None
    } else if music_path.exists() {
        return Err("Music.dat exists but is not a valid Doraemon local-music file; move it aside before applying local music".into());
    } else if cue::valid_wav(&wav_path) {
        Some((wav_path, true))
    } else if let Some(cue_path) = options.cue.as_ref().filter(|path| cue::valid_cue(path)) {
        Some((cue_path.clone(), false))
    } else {
        progress(
            sink,
            TaskState::Skipped,
            "Local music was requested, but no Music.dat, verified WAV, or CUE/BIN was found. The original music code was left untouched.",
            Some(45),
        );
        return Ok(LocalAudioPreparation {
            enabled: false,
            summary: "No local music source was available, so music playback was left unchanged."
                .into(),
            created: Vec::new(),
        });
    };
    let helper = bundled_audio_helper(payload)
        .ok_or("this patcher does not include doraudio.dll, so local music cannot be enabled")?;
    let helper_path = folder.join("doraudio.dll");
    if helper_path.exists() && hash::file(&helper_path)? != helper.hash {
        return Err("doraudio.dll already exists and differs from this patcher; move it aside before enabling local music".into());
    }
    let mut created = Vec::new();
    if let Some((source_path, is_wav)) = source {
        progress(
            sink,
            TaskState::Working,
            if is_wav {
                "Compressing DoraemonMusic.wav into Music.dat…"
            } else {
                "Reading the disc image and building Music.dat…"
            },
            Some(43),
        );
        let staged = staging.join("Music.dat");
        if is_wav {
            music::encode_wav(&source_path, &staged)?;
        } else {
            music::encode_cue(&source_path, &staged)?;
        }
        let digest = hash::file(&staged)?;
        created.push(("Music.dat".into(), staged, digest));
    }
    if !helper_path.exists() {
        let staged = staging.join("doraudio.dll");
        write_synced(&staged, &helper.bytes)?;
        created.push(("doraudio.dll".into(), staged, helper.hash));
    }
    progress(
        sink,
        TaskState::Done,
        "Local DirectSound music is ready.",
        Some(47),
    );
    Ok(LocalAudioPreparation {
        enabled: true,
        summary: "Music.dat will play through the local DirectSound backend.".into(),
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
    let result = pe::patch_compatible(
        &original,
        options.no_disc,
        local_audio.enabled,
        options.no_reg,
        options.modern_volume,
    )?;
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
    fs::copy(&exe_path, backup.join("original/Doraemon.exe"))
        .map_err(|e| format!("backup Doraemon.exe: {e}"))?;
    fs::copy(patcher_exe, backup.join("Restore.exe"))
        .map_err(|e| format!("create Restore.exe: {e}"))?;
    let original_hash = hash::bytes(&original);
    let created_files: Vec<_> = local_audio
        .created
        .iter()
        .map(|(name, _, digest)| (name.clone(), *digest))
        .collect();
    let manifest = backup_manifest(
        payload.language.label(),
        &[("Doraemon.exe".into(), original_hash)],
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
        if voice::matches(&source, voice_patch) {
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
            let output = voice::apply_patch(&source, voice_patch)?;
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

    let local_audio = prepare_local_audio(folder, &staging, payload, options, sink)?;
    let exe_path = find_file(folder, "Doraemon.exe")?;
    let exe_source =
        fs::read(&exe_path).map_err(|error| format!("{}: {error}", exe_path.display()))?;
    progress(
        sink,
        TaskState::Working,
        "Checking the game executable structure…",
        Some(40),
    );
    let exe_patch = pe::patch_language_runtime(
        &exe_source,
        payload.language == Language::Vietnamese,
        options.no_disc,
        options.no_reg,
        local_audio.enabled,
        options.modern_volume,
    )?;
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
        for (name, _, _, _) in &generated {
            if !originals.contains_key(name) {
                return Err(format!(
                    "the existing backup does not contain an original {name}; restore before applying this additional change"
                ));
            }
        }
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
    } else {
        progress(
            sink,
            TaskState::Working,
            "Creating your original-file backup…",
            Some(65),
        );
        fs::create_dir_all(backup.join("original")).map_err(|error| error.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::{BundledFile, Language, PatchProfile};

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
    fn local_music_files_are_never_staged_when_option_is_off() {
        let folder = std::env::temp_dir().join(format!(
            "doraemon-local-music-off-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        let staging = folder.join("staging");
        fs::create_dir_all(&staging).unwrap();
        let helper = b"helper".to_vec();
        let payload = Payload {
            language: Language::Custom,
            profiles: Vec::new(),
            strings: None,
            voice: None,
            bundled: vec![BundledFile {
                name: "doraudio.dll".into(),
                hash: hash::bytes(&helper),
                bytes: helper,
            }],
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
        assert!(!staging.join("doraudio.dll").exists());
        assert!(!staging.join("Music.dat").exists());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn backup_manifest_tracks_all_generated_local_music_files() {
        let music = hash::bytes(b"music");
        let helper = hash::bytes(b"helper");
        let manifest = backup_manifest(
            "test",
            &[],
            &[("Music.dat".into(), music), ("doraudio.dll".into(), helper)],
        );
        let created = manifest_created_files(&manifest).unwrap();
        assert_eq!(created.get("Music.dat"), Some(&music));
        assert_eq!(created.get("doraudio.dll"), Some(&helper));
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
            },
            &std::env::current_exe().unwrap(),
        )
        .unwrap();
        assert!(!reapplied.changed.is_empty());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn real_local_music_installs_and_restores_when_fixtures_are_available() {
        let (Ok(base), Ok(payload_path), Ok(cue_path), Ok(helper_path)) = (
            std::env::var("DORAEMON_TEST_DATA_DIR"),
            std::env::var("DORAEMON_TEST_PAYLOAD"),
            std::env::var("DORAEMON_TEST_CUE"),
            std::env::var("DORAEMON_TEST_AUDIO_HELPER"),
        ) else {
            return;
        };
        let mut payload = crate::payload::decode(&fs::read(payload_path).unwrap()).unwrap();
        let helper = fs::read(helper_path).unwrap();
        payload.bundled.push(BundledFile {
            name: "doraudio.dll".into(),
            hash: hash::bytes(&helper),
            bytes: helper,
        });
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
                modern_volume: false,
                cue: Some(PathBuf::from(cue_path)),
            },
            &std::env::current_exe().unwrap(),
        )
        .unwrap();
        assert!(report.changed.iter().any(|name| name == "Music.dat"));
        assert!(report.changed.iter().any(|name| name == "doraudio.dll"));
        assert!(music::valid(&folder.join("Music.dat")));
        assert!(folder.join("doraudio.dll").is_file());
        restore(&folder.join("backup")).unwrap();
        assert!(!folder.join("Music.dat").exists());
        assert!(!folder.join("doraudio.dll").exists());
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
        fs::remove_dir_all(folder).unwrap();
    }
}

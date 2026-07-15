use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    cue, hash,
    payload::{FilePatch, Payload},
    Result,
};

#[derive(Clone, Debug, Default)]
pub struct ApplyOptions {
    pub no_disc: bool,
    pub cue: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct ApplyReport {
    pub changed: Vec<String>,
    pub audio: String,
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

fn selected_patches<'a>(payload: &'a Payload, no_disc: bool) -> Vec<&'a FilePatch> {
    let mut output: Vec<_> = payload.files.iter().collect();
    if no_disc {
        output.push(&payload.executable_portable);
    } else if let Some(executable) = &payload.executable_plain {
        output.push(executable);
    }
    output
}

fn backup_manifest(
    language: &str,
    originals: &[(String, hash::Hash)],
    created_audio: Option<hash::Hash>,
) -> String {
    let mut output =
        format!("{{\n  \"version\": 1,\n  \"language\": \"{language}\",\n  \"files\": {{\n");
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
    output.push_str("  },\n  \"created_audio\": ");
    if let Some(digest) = created_audio {
        output.push_str(&format!(
            "{{ \"name\": \"DoraemonMusic.wav\", \"sha256\": \"{}\" }}\n",
            hash::hex(&digest)
        ));
    } else {
        output.push_str("null\n");
    }
    output.push_str("}\n");
    output
}

pub fn apply(
    folder: &Path,
    payload: &Payload,
    options: &ApplyOptions,
    patcher_exe: &Path,
) -> Result<ApplyReport> {
    if !folder.is_dir() {
        return Err(format!("{} is not a game folder", folder.display()));
    }
    let backup = folder.join("backup");
    if backup.exists() {
        return Err(
            "backup already exists; run backup/Restore.exe before applying another patch".into(),
        );
    }

    let mut resolved = Vec::new();
    let mut base_ok = true;
    let mut target_ok = true;
    for required in &payload.required {
        let path = find_file(folder, &required.name)?;
        let digest = hash::file(&path)?;
        base_ok &= digest == required.hash
            && fs::metadata(&path)
                .map_err(|error| error.to_string())?
                .len()
                == required.len;
        if let Some(patch) = selected_patches(payload, options.no_disc)
            .iter()
            .find(|patch| patch.name.eq_ignore_ascii_case(&required.name))
        {
            target_ok &= digest == patch.target_hash;
        } else {
            target_ok &= digest == required.hash;
        }
        resolved.push((required.name.clone(), path, digest));
    }
    if target_ok {
        return Err(format!(
            "the {} patch is already installed",
            payload.language.label()
        ));
    }
    if !base_ok {
        return Err("the selected folder is modified or is not the supported Cantonese release; no files were changed".into());
    }

    let patches = selected_patches(payload, options.no_disc);
    let staging = folder.join(".doraemon-patch-staging");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&staging).map_err(|error| error.to_string())?;
    let mut generated = Vec::new();
    for patch in &patches {
        let source_path = find_file(folder, &patch.name)?;
        let source = fs::read(&source_path)
            .map_err(|error| format!("{}: {error}", source_path.display()))?;
        let output = patch.apply(&source)?;
        let temporary = staging.join(&patch.name);
        write_synced(&temporary, &output)?;
        generated.push((
            patch.name.clone(),
            source_path,
            temporary,
            patch.target_hash,
        ));
    }

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

    let wav = folder.join("DoraemonMusic.wav");
    let mut created_audio = None;
    let audio = if options.no_disc && cue::valid_wav(&wav) {
        "Using existing verified DoraemonMusic.wav.".to_string()
    } else if options.no_disc {
        if let Some(cue_path) = &options.cue {
            cue::extract(cue_path, &wav)?;
            let digest = hash::file(&wav)?;
            created_audio = Some(digest);
            "Extracted DoraemonMusic.wav from the supplied disc image.".into()
        } else {
            "No valid WAV or CUE was supplied. The patched game will continue silently.".into()
        }
    } else {
        "Original CD and registry behavior retained.".into()
    };

    let manifest = backup_manifest(payload.language.label(), &originals, created_audio);
    write_synced(&backup.join("manifest.json"), manifest.as_bytes())?;
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
    let _ = fs::remove_dir(&staging);
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
    let mut created_audio = None;
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
        } else if trimmed.starts_with("\"created_audio\": {") {
            let name = trimmed
                .split("\"name\": \"")
                .nth(1)
                .and_then(|value| value.split('"').next())
                .ok_or("invalid audio manifest")?;
            let digest = trimmed
                .split("\"sha256\": \"")
                .nth(1)
                .and_then(|value| value.split('"').next())
                .ok_or("invalid audio manifest")?;
            created_audio = Some((name.to_string(), hash::parse(digest)?));
        }
    }
    if let Some((name, digest)) = created_audio {
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
        for required in &payload.required {
            fs::copy(
                Path::new(&base).join(&required.name),
                folder.join(&required.name),
            )
            .unwrap();
        }
        let before: Vec<_> = payload
            .required
            .iter()
            .map(|required| {
                (
                    required.name.clone(),
                    hash::file(&folder.join(&required.name)).unwrap(),
                )
            })
            .collect();
        let report = apply(
            &folder,
            &payload,
            &ApplyOptions {
                no_disc: false,
                cue: None,
            },
            &std::env::current_exe().unwrap(),
        )
        .unwrap();
        assert!(!report.changed.is_empty());
        assert!(folder.join("backup/Restore.exe").exists());
        assert!(folder.join("backup/manifest.json").exists());
        restore(&folder.join("backup")).unwrap();
        for (name, digest) in before {
            assert_eq!(hash::file(&folder.join(name)).unwrap(), digest);
        }
        fs::remove_dir_all(folder).unwrap();
    }
}

//! Language packs.
//!
//! The patch data used to be zstd-compressed into `patcher.exe` at build time,
//! which made every download the size of both languages together. It now ships
//! as one release asset per language, produced by `patch-build pack`:
//!
//! * `<language>-pack.bin` - a zstd-compressed DPART container holding that
//!   language's `.dmpatch` parts,
//! * `<language>-pack.bin.sha256` - its digest, also listed in the release notes.
//!
//! [`resolve`] hands back the payload from disk when a pack is installed next to
//! the executable or already cached, and downloads the pack published for this
//! exact patcher version otherwise.

// Downloading is Windows-only, so on other hosts only the pack format itself
// (and its tests) is compiled.
#![cfg_attr(not(windows), allow(dead_code))]

use doraemon_game_patch::{
    hash,
    payload::{self, Language, Payload},
};
use std::path::{Path, PathBuf};

/// Packs live on the release whose tag matches the build that expects them.
const REPOSITORY: &str = "lhuthng/doraemon-monopoly-localization";
const PACK_MAGIC: &[u8; 4] = b"DZC1";

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn name(language: Language) -> &'static str {
    match language {
        Language::English => "english",
        Language::Vietnamese => "vietnamese",
        Language::Custom => "custom",
    }
}

fn label(language: Language) -> &'static str {
    match language {
        Language::English => "English",
        Language::Vietnamese => "Vietnamese",
        Language::Custom => "custom",
    }
}

pub fn asset(language: Language) -> String {
    format!("{}-pack.bin", name(language))
}

pub fn url(language: Language) -> String {
    format!(
        "https://github.com/{REPOSITORY}/releases/download/patcher-v{VERSION}/{}",
        asset(language)
    )
}

/// Where a downloaded pack is kept between runs.
fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("DoraemonPatcher").join("packs").join(VERSION))
}

/// A pack that is already on disk: an explicit override first (handy for
/// testing and for preloading offline), then a `packs` folder beside the
/// executable, then the per-user cache.
pub fn installed(language: Language) -> Option<PathBuf> {
    let asset = asset(language);
    let mut candidates = Vec::new();
    if let Some(folder) = std::env::var_os("DORAEMON_PATCHER_PACK_DIR") {
        candidates.push(PathBuf::from(folder).join(&asset));
    }
    if let Some(folder) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        candidates.push(folder.join("packs").join(&asset));
    }
    if let Some(folder) = cache_dir() {
        candidates.push(folder.join(&asset));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 12 || &data[..4] != PACK_MAGIC {
        return Err("this file is not a language pack".into());
    }
    let original = u64::from_le_bytes(data[4..12].try_into().unwrap());
    zstd::bulk::decompress(&data[12..], original as usize)
        .map_err(|error| format!("the language pack failed to decompress ({error})"))
}

/// Splits a DPART container into its raw part blobs.
fn split_parts(blob: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if blob.len() < 7 || &blob[..5] != b"DPART" {
        return Err("the language pack has the wrong container header".into());
    }
    let count = u16::from_le_bytes(blob[5..7].try_into().unwrap()) as usize;
    if blob.len() < 7 + count * 8 {
        return Err("the language pack has a truncated part table".into());
    }
    let mut parts = Vec::with_capacity(count);
    for index in 0..count {
        let entry = 7 + index * 8;
        let start = u32::from_le_bytes(blob[entry..entry + 4].try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(blob[entry + 4..entry + 8].try_into().unwrap()) as usize;
        let bytes = blob
            .get(start..start + len)
            .ok_or("the language pack has a part past its end")?;
        parts.push(bytes.to_vec());
    }
    Ok(parts)
}

pub fn decode(bytes: &[u8], language: Language) -> Result<Payload, String> {
    let blob = decompress(bytes)?;
    let mut parts = Vec::new();
    for part in split_parts(&blob)? {
        if part.is_empty() {
            continue;
        }
        parts.push(payload::decode_part(&part).map_err(|error| {
            format!("the {} language pack is corrupt ({error})", label(language))
        })?);
    }
    if parts.is_empty() {
        return Err(format!("the {} language pack is empty", label(language)));
    }
    let payload = doraemon_game_patch::merge_parts(&parts)?;
    if payload.language != language {
        return Err(format!(
            "this is the {} pack, not the {} one",
            name(payload.language),
            name(language)
        ));
    }
    Ok(payload)
}

/// The payload for a language that is already on disk, if any. Never touches
/// the network, so it is safe to call while the window is coming up.
pub fn cached(language: Language) -> Option<Payload> {
    let path = installed(language)?;
    match std::fs::read(&path)
        .map_err(|error| error.to_string())
        .and_then(|bytes| decode(&bytes, language))
    {
        Ok(payload) => Some(payload),
        Err(error) => {
            eprintln!(
                "DIAG: ignoring {} ({error}); it will be downloaded again",
                path.display()
            );
            None
        }
    }
}

/// Resolves a language, downloading and verifying its pack the first time.
pub fn resolve(language: Language, report: &mut dyn FnMut(&str)) -> Result<Payload, String> {
    if let Some(payload) = cached(language) {
        return Ok(payload);
    }
    let asset = asset(language);
    let folder = cache_dir().ok_or("no folder is available to cache language packs")?;
    std::fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let destination = folder.join(&asset);
    let url = url(language);

    report(&format!(
        "Downloading the {} language pack. This happens once…",
        label(language)
    ));
    download(&format!("{url}.sha256"), &destination.with_extension("sha256"))
        .map_err(|error| format!("{error}\nExpected {url}.sha256"))?;
    download(&url, &destination).map_err(|error| format!("{error}\nExpected {url}"))?;
    let expected = read_digest(&destination.with_extension("sha256"))?;
    let found = hash::hex(&hash::file(&destination).map_err(|error| error.to_string())?);
    if expected != found {
        let _ = std::fs::remove_file(&destination);
        return Err(format!(
            "the downloaded {} pack failed its checksum (expected {expected}, got {found})",
            label(language)
        ));
    }
    report(&format!("Checking the {} language pack…", label(language)));
    decode(&std::fs::read(&destination).map_err(|error| error.to_string())?, language)
}

/// Reads the hex digest from a `.sha256` sidecar.
fn read_digest(path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let digest = text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{} does not hold a checksum", path.display()));
    }
    Ok(digest)
}

fn download(url: &str, destination: &Path) -> Result<(), String> {
    let partial = destination.with_extension("part");
    let _ = std::fs::remove_file(&partial);
    fetch(url, &partial)?;
    std::fs::rename(&partial, destination)
        .map_err(|error| format!("{}: {error}", destination.display()))
}

// `winapi` only ships the `urlmon` interfaces, so the one download call is
// declared here. It writes straight to disk, follows the GitHub redirect to
// the asset host, and uses the system proxy and certificate store.
#[cfg(windows)]
#[link(name = "urlmon")]
extern "system" {
    fn URLDownloadToFileW(
        caller: *mut core::ffi::c_void,
        url: *const u16,
        file_name: *const u16,
        reserved: u32,
        status: *mut core::ffi::c_void,
    ) -> i32;
}

#[cfg(windows)]
fn fetch(url: &str, destination: &Path) -> Result<(), String> {
    use std::{iter, os::windows::ffi::OsStrExt};

    let url: Vec<u16> = url.encode_utf16().chain(iter::once(0)).collect();
    let path: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let code = unsafe {
        URLDownloadToFileW(
            std::ptr::null_mut(),
            url.as_ptr(),
            path.as_ptr(),
            0,
            std::ptr::null_mut(),
        )
    };
    if code == 0 {
        Ok(())
    } else {
        Err(format!("the download failed (HRESULT 0x{code:08X})"))
    }
}

#[cfg(not(windows))]
fn fetch(_url: &str, _destination: &Path) -> Result<(), String> {
    Err("only the Windows patcher downloads language packs".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use doraemon_game_patch::payload::PayloadPart;

    /// Every published pack must name the release that this build was tagged
    /// for, otherwise a patcher update would keep fetching an older payload.
    #[test]
    fn pack_urls_point_at_this_version() {
        assert_eq!(asset(Language::English), "english-pack.bin");
        assert!(url(Language::Vietnamese).ends_with("/vietnamese-pack.bin"));
        assert!(url(Language::English).contains(&format!("patcher-v{VERSION}")));
    }

    fn sample_pack(language: Language) -> Vec<u8> {
        let part = PayloadPart {
            format_version: 4,
            language,
            target: doraemon_game_patch::payload::TargetName::Runtime,
            base_fingerprints: Vec::new(),
            strings: None,
            voice: None,
            file_patches: Vec::new(),
            executable_plain: None,
            executable_portable: None,
            bundled: Vec::new(),
            is_empty: false,
        };
        let part = payload::encode_part(&part).expect("encode part");
        let mut blob = Vec::from(&b"DPART"[..]);
        blob.extend_from_slice(&1u16.to_le_bytes());
        let start = (7 + 8) as u32;
        blob.extend_from_slice(&start.to_le_bytes());
        blob.extend_from_slice(&(part.len() as u32).to_le_bytes());
        blob.extend_from_slice(&part);

        let mut packed = Vec::from(&PACK_MAGIC[..]);
        packed.extend_from_slice(&(blob.len() as u64).to_le_bytes());
        packed.extend_from_slice(&zstd::bulk::compress(&blob, 19).unwrap());
        packed
    }

    #[test]
    fn a_pack_round_trips_into_its_language() {
        let bytes = sample_pack(Language::English);
        let payload = decode(&bytes, Language::English).expect("decode pack");
        assert_eq!(payload.language, Language::English);
    }

    /// A Vietnamese download must not be accepted as the English payload.
    #[test]
    fn a_pack_of_another_language_is_rejected() {
        let bytes = sample_pack(Language::Vietnamese);
        let error = decode(&bytes, Language::English).expect_err("wrong language must fail");
        assert!(error.contains("vietnamese"), "{error}");
    }

    #[test]
    fn foreign_bytes_are_not_a_pack() {
        assert!(decode(b"not a pack at all", Language::English).is_err());
        let mut truncated = sample_pack(Language::English);
        truncated.truncate(20);
        assert!(decode(&truncated, Language::English).is_err());
    }
}

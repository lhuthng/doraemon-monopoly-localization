use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use crate::Result;

pub const WAV_BYTES: u64 = 169_179_404;
const SECTOR_BYTES: u64 = 2_352;
const FRAMES_PER_SECOND: u64 = 75;
const EXPECTED_BIN_BYTES: u64 = 409_701_936;
const EXPECTED_AUDIO_START_FRAME: u64 = 102_263;
const EXPECTED_AUDIO_BYTES: u64 = 169_179_360;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Track {
    pub number: u8,
    pub kind: String,
    pub frame: u64,
}

#[derive(Clone, Debug)]
pub struct Cue {
    pub bin_path: PathBuf,
    pub tracks: Vec<Track>,
}

fn timestamp(value: &str) -> Result<u64> {
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("invalid CUE timestamp {value}"));
    }
    let minutes: u64 = parts[0]
        .parse()
        .map_err(|_| format!("invalid CUE timestamp {value}"))?;
    let seconds: u64 = parts[1]
        .parse()
        .map_err(|_| format!("invalid CUE timestamp {value}"))?;
    let frames: u64 = parts[2]
        .parse()
        .map_err(|_| format!("invalid CUE timestamp {value}"))?;
    if seconds >= 60 || frames >= FRAMES_PER_SECOND {
        return Err(format!("invalid CUE timestamp {value}"));
    }
    Ok((minutes * 60 + seconds) * FRAMES_PER_SECOND + frames)
}

pub fn parse(path: &Path) -> Result<Cue> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut bin = None;
    let mut current: Option<(u8, String)> = None;
    let mut tracks = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with("FILE ") && upper.ends_with(" BINARY") {
            let first = trimmed
                .find('"')
                .ok_or("CUE FILE must quote its BIN filename")?;
            let last = trimmed[first + 1..]
                .find('"')
                .ok_or("CUE FILE must quote its BIN filename")?
                + first
                + 1;
            bin = Some(trimmed[first + 1..last].to_string());
        } else if upper.starts_with("TRACK ") {
            let fields: Vec<_> = trimmed.split_whitespace().collect();
            if fields.len() != 3 {
                return Err(format!("invalid CUE track line {trimmed}"));
            }
            current = Some((
                fields[1]
                    .parse()
                    .map_err(|_| format!("invalid track number in {trimmed}"))?,
                fields[2].to_ascii_uppercase(),
            ));
        } else if upper.starts_with("INDEX 01 ") {
            if let Some((number, kind)) = &current {
                let value = trimmed
                    .split_whitespace()
                    .nth(2)
                    .ok_or("invalid INDEX 01")?;
                tracks.push(Track {
                    number: *number,
                    kind: kind.clone(),
                    frame: timestamp(value)?,
                });
            }
        }
    }
    let bin = bin.ok_or("CUE must contain one quoted BINARY file")?;
    Ok(Cue {
        bin_path: path.parent().unwrap_or(Path::new(".")).join(bin),
        tracks,
    })
}

fn validate(cue: &Cue) -> Result<()> {
    let bin_bytes = fs::metadata(&cue.bin_path)
        .map_err(|error| format!("{}: {error}", cue.bin_path.display()))?
        .len();
    if bin_bytes != EXPECTED_BIN_BYTES {
        return Err(format!(
            "unexpected BIN size {bin_bytes}; expected {EXPECTED_BIN_BYTES}"
        ));
    }
    if cue.tracks.len() != 11 {
        return Err(format!("expected 11 tracks; found {}", cue.tracks.len()));
    }
    for (index, track) in cue.tracks.iter().enumerate() {
        if track.number as usize != index + 1 {
            return Err("CUE track numbers are not sequential".into());
        }
    }
    if cue.tracks[0].kind != "MODE1/2352" || cue.tracks[0].frame != 0 {
        return Err("track 1 is not MODE1/2352 at frame zero".into());
    }
    if cue.tracks[1..].iter().any(|track| track.kind != "AUDIO") {
        return Err("tracks 2 through 11 must be AUDIO".into());
    }
    if cue.tracks[1].frame != EXPECTED_AUDIO_START_FRAME {
        return Err(format!(
            "unexpected first audio frame {}",
            cue.tracks[1].frame
        ));
    }
    Ok(())
}

fn header() -> [u8; 44] {
    let mut output = [0_u8; 44];
    output[0..4].copy_from_slice(b"RIFF");
    output[4..8].copy_from_slice(&((EXPECTED_AUDIO_BYTES + 36) as u32).to_le_bytes());
    output[8..16].copy_from_slice(b"WAVEfmt ");
    output[16..20].copy_from_slice(&16_u32.to_le_bytes());
    output[20..22].copy_from_slice(&1_u16.to_le_bytes());
    output[22..24].copy_from_slice(&2_u16.to_le_bytes());
    output[24..28].copy_from_slice(&44_100_u32.to_le_bytes());
    output[28..32].copy_from_slice(&176_400_u32.to_le_bytes());
    output[32..34].copy_from_slice(&4_u16.to_le_bytes());
    output[34..36].copy_from_slice(&16_u16.to_le_bytes());
    output[36..40].copy_from_slice(b"data");
    output[40..44].copy_from_slice(&(EXPECTED_AUDIO_BYTES as u32).to_le_bytes());
    output
}

pub fn valid_wav(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut bytes = [0_u8; 44];
    file.read_exact(&mut bytes).is_ok()
        && bytes == header()
        && file
            .metadata()
            .map(|meta| meta.len() == WAV_BYTES)
            .unwrap_or(false)
}

pub fn valid_cue(path: &Path) -> bool {
    parse(path).and_then(|cue| validate(&cue)).is_ok()
}

pub fn extract(cue_path: &Path, output: &Path) -> Result<()> {
    let cue = parse(cue_path)?;
    validate(&cue)?;
    let mut source = File::open(&cue.bin_path)
        .map_err(|error| format!("{}: {error}", cue.bin_path.display()))?;
    source
        .seek(SeekFrom::Start(EXPECTED_AUDIO_START_FRAME * SECTOR_BYTES))
        .map_err(|error| error.to_string())?;
    let temporary = output.with_extension("wav.dmpatch.tmp");
    let mut target =
        File::create(&temporary).map_err(|error| format!("{}: {error}", temporary.display()))?;
    target
        .write_all(&header())
        .map_err(|error| error.to_string())?;
    let mut limited = source.take(EXPECTED_AUDIO_BYTES);
    let copied = std::io::copy(&mut limited, &mut target).map_err(|error| error.to_string())?;
    if copied != EXPECTED_AUDIO_BYTES {
        let _ = fs::remove_file(&temporary);
        return Err(format!("BIN ended after {copied} audio bytes"));
    }
    target.sync_all().map_err(|error| error.to_string())?;
    drop(target);
    if !valid_wav(&temporary) {
        let _ = fs::remove_file(&temporary);
        return Err("extracted WAV verification failed".into());
    }
    crate::install::replace_file(&temporary, output)?;
    Ok(())
}

pub fn track_milliseconds(cue_path: &Path) -> Result<Vec<(u32, u32)>> {
    let cue = parse(cue_path)?;
    validate(&cue)?;
    Ok(cue.tracks[1..]
        .iter()
        .enumerate()
        .map(|(index, track)| {
            let end = cue
                .tracks
                .get(index + 2)
                .map(|next| next.frame)
                .unwrap_or(EXPECTED_BIN_BYTES / SECTOR_BYTES);
            (
                ((track.frame - EXPECTED_AUDIO_START_FRAME) * 1000 / FRAMES_PER_SECOND) as u32,
                ((end - EXPECTED_AUDIO_START_FRAME) * 1000 / FRAMES_PER_SECOND) as u32,
            )
        })
        .collect())
}

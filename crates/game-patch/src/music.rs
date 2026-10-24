use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::{install::replace_file, Result};

pub const MAGIC: &[u8; 8] = b"DBGM1\0\0\0";
/// Version 1 is IMA ADPCM in 4096-frame blocks; version 2 is raw 16-bit PCM.
/// The embedded runtime decodes the first in the guest and streams the second
/// straight into the DirectSound buffer, which keeps the music off the
/// interpreter's instruction budget.
pub const VERSION_IMA: u32 = 1;
pub const VERSION_PCM: u32 = 2;
pub const HEADER_SIZE: usize = 192;
pub const TRACK_COUNT: usize = 10;
pub const SAMPLE_RATE: u32 = 22_050;
pub const BLOCK_FRAMES: usize = 4_096;
pub const TRACK_FRAMES: [u32; TRACK_COUNT] = [
    7_437_612, 2_821_812, 880_824, 2_350_824, 129_948, 2_086_224, 1_497_636, 1_332_702, 1_859_844,
    264_894,
];

/// Each disc audio track ends with 4 seconds of spill (2 seconds of silence
/// followed by 2 seconds of the following track's opening). The game BGM
/// trims that spill and reuses the previous track's trailing 2 seconds as the
/// current track's opening, leaving a 2-second gap between the encoded track
/// regions in the source stream that the encoder must skip.
const SEGUE_BYTES: u64 = (44_100 * 2) as u64 * 4;

const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
const INDEX_TABLE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn get_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(input[offset..offset + 2].try_into().unwrap())
}

fn get_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap())
}

fn encode_nibble(sample: i16, predictor: &mut i32, index: &mut i32) -> u8 {
    let step = STEP_TABLE[*index as usize];
    let difference = sample as i32 - *predictor;
    let mut magnitude = difference.unsigned_abs() as i32;
    let mut nibble = if difference < 0 { 8 } else { 0 };
    let mut reconstructed = step >> 3;
    if magnitude >= step {
        nibble |= 4;
        magnitude -= step;
        reconstructed += step;
    }
    if magnitude >= step >> 1 {
        nibble |= 2;
        magnitude -= step >> 1;
        reconstructed += step >> 1;
    }
    if magnitude >= step >> 2 {
        nibble |= 1;
        reconstructed += step >> 2;
    }
    if nibble & 8 != 0 {
        *predictor -= reconstructed;
    } else {
        *predictor += reconstructed;
    }
    *predictor = (*predictor).clamp(i16::MIN as i32, i16::MAX as i32);
    *index = (*index + INDEX_TABLE[nibble as usize]).clamp(0, 88);
    nibble
}

fn mono_sample(source: &[u8]) -> i16 {
    let samples = source.len() / 2;
    let sum: i64 = source
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]) as i64)
        .sum();
    (sum / samples as i64).clamp(i16::MIN as i64, i16::MAX as i64) as i16
}

fn encode_track<R: Read>(
    source: &mut R,
    target: &mut File,
    frames: u32,
    source_frames_per_output: usize,
) -> Result<u32> {
    let mut remaining = frames as usize;
    let mut written = 0_u32;
    while remaining != 0 {
        let count = remaining.min(BLOCK_FRAMES);
        let bytes_per_output = source_frames_per_output * 4;
        let mut pcm = vec![0_u8; count * bytes_per_output];
        source
            .read_exact(&mut pcm)
            .map_err(|error| format!("read source PCM: {error}"))?;
        let first = mono_sample(&pcm[..bytes_per_output]);
        let mut block = Vec::with_capacity(6 + count.div_ceil(2));
        block.extend_from_slice(&(count as u16).to_le_bytes());
        block.extend_from_slice(&first.to_le_bytes());
        block.extend_from_slice(&[0, 0]);
        let mut predictor = first as i32;
        let mut step_index = 0;
        let mut packed = 0_u8;
        for frame in 1..count {
            let at = frame * bytes_per_output;
            let sample = mono_sample(&pcm[at..at + bytes_per_output]);
            let nibble = encode_nibble(sample, &mut predictor, &mut step_index);
            if frame & 1 == 1 {
                packed = nibble;
            } else {
                block.push(packed | (nibble << 4));
            }
        }
        if count & 1 == 0 {
            block.push(packed);
        }
        target
            .write_all(&block)
            .map_err(|error| format!("write BGM.dat block: {error}"))?;
        written = written
            .checked_add(block.len() as u32)
            .ok_or("BGM.dat track exceeds 4 GiB")?;
        remaining -= count;
    }
    Ok(written)
}

fn encode_reader<R: Read + Seek>(
    source: &mut R,
    output: &Path,
    source_frames_per_output: u32,
) -> Result<()> {
    let sample_rate = 44_100 / source_frames_per_output;
    let track_frames: Vec<u32> = TRACK_FRAMES
        .iter()
        .map(|frames| frames * 2 / source_frames_per_output)
        .collect();
    if track_frames.len() != TRACK_COUNT {
        return Err("BGM.dat requires exactly ten tracks".into());
    }
    let temporary = output.with_extension("dat.dmpatch.tmp");
    let mut target =
        File::create(&temporary).map_err(|error| format!("{}: {error}", temporary.display()))?;
    target
        .write_all(&[0_u8; HEADER_SIZE])
        .map_err(|error| error.to_string())?;
    let mut entries = Vec::with_capacity(TRACK_COUNT);
    for (index, frames) in track_frames.iter().copied().enumerate() {
        let offset = target
            .stream_position()
            .map_err(|error| error.to_string())? as u32;
        let length = encode_track(
            source,
            &mut target,
            frames,
            source_frames_per_output as usize,
        )?;
        entries.push((index as u32 + 2, offset, length, frames));
        if index + 1 != TRACK_COUNT {
            source
                .seek(SeekFrom::Current(SEGUE_BYTES as i64))
                .map_err(|error| error.to_string())?;
        }
    }
    let mut header = [0_u8; HEADER_SIZE];
    header[..8].copy_from_slice(MAGIC);
    put_u32(&mut header, 8, 1);
    put_u32(&mut header, 12, TRACK_COUNT as u32);
    put_u32(&mut header, 16, sample_rate);
    put_u16(&mut header, 20, 1);
    put_u16(&mut header, 22, 16);
    put_u32(&mut header, 24, BLOCK_FRAMES as u32);
    for (index, (id, offset, length, frames)) in entries.into_iter().enumerate() {
        let at = 32 + index * 16;
        put_u32(&mut header, at, id);
        put_u32(&mut header, at + 4, offset);
        put_u32(&mut header, at + 8, length);
        put_u32(&mut header, at + 12, frames);
    }
    target
        .seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    target
        .write_all(&header)
        .map_err(|error| error.to_string())?;
    target.sync_all().map_err(|error| error.to_string())?;
    drop(target);
    if !valid(&temporary) {
        let _ = fs::remove_file(&temporary);
        return Err("generated BGM.dat failed verification".into());
    }
    replace_file(&temporary, output)
}

fn track_frames_for(source_frames_per_output: u32) -> Vec<u32> {
    TRACK_FRAMES
        .iter()
        .map(|frames| frames * 2 / source_frames_per_output)
        .collect()
}

fn write_header(
    header: &mut [u8],
    version: u32,
    sample_rate: u32,
    entries: &[(u32, u32, u32, u32)],
) {
    header[..8].copy_from_slice(MAGIC);
    put_u32(header, 8, version);
    put_u32(header, 12, TRACK_COUNT as u32);
    put_u32(header, 16, sample_rate);
    put_u16(header, 20, 1);
    put_u16(header, 22, 16);
    put_u32(header, 24, BLOCK_FRAMES as u32);
    for (index, (id, offset, length, frames)) in entries.iter().enumerate() {
        let at = 32 + index * 16;
        put_u32(header, at, *id);
        put_u32(header, at + 4, *offset);
        put_u32(header, at + 8, *length);
        put_u32(header, at + 12, *frames);
    }
}

/// Writes the header over the placeholder, verifies the result, and puts it in
/// place next to the sources.
fn finish_container(
    mut target: File,
    temporary: &Path,
    output: &Path,
    version: u32,
    sample_rate: u32,
    entries: &[(u32, u32, u32, u32)],
) -> Result<()> {
    let mut header = [0_u8; HEADER_SIZE];
    write_header(&mut header, version, sample_rate, entries);
    target
        .seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    target
        .write_all(&header)
        .map_err(|error| error.to_string())?;
    target.sync_all().map_err(|error| error.to_string())?;
    drop(target);
    if !valid(temporary) {
        let _ = fs::remove_file(temporary);
        return Err("generated BGM.dat failed verification".into());
    }
    replace_file(temporary, output)
}

/// Writes one raw-PCM track, downmixing the stereo source to mono.
fn encode_track_pcm<R: Read>(
    source: &mut R,
    target: &mut File,
    frames: u32,
    source_frames_per_output: usize,
) -> Result<u32> {
    let mut remaining = frames as usize;
    let mut written = 0_u32;
    let mut block = Vec::with_capacity(BLOCK_FRAMES * 2);
    while remaining != 0 {
        let count = remaining.min(BLOCK_FRAMES);
        let bytes_per_output = source_frames_per_output * 4;
        let mut pcm = vec![0_u8; count * bytes_per_output];
        source
            .read_exact(&mut pcm)
            .map_err(|error| format!("read source PCM: {error}"))?;
        block.clear();
        for frame in 0..count {
            let at = frame * bytes_per_output;
            block.extend_from_slice(&mono_sample(&pcm[at..at + bytes_per_output]).to_le_bytes());
        }
        target
            .write_all(&block)
            .map_err(|error| format!("write BGM.dat PCM: {error}"))?;
        written = written
            .checked_add(block.len() as u32)
            .ok_or("BGM.dat track exceeds 4 GiB")?;
        remaining -= count;
    }
    Ok(written)
}

fn encode_reader_pcm<R: Read + Seek>(
    source: &mut R,
    output: &Path,
    source_frames_per_output: u32,
) -> Result<()> {
    let sample_rate = 44_100 / source_frames_per_output;
    let track_frames = track_frames_for(source_frames_per_output);
    let temporary = output.with_extension("dat.dmpatch.tmp");
    let mut target =
        File::create(&temporary).map_err(|error| format!("{}: {error}", temporary.display()))?;
    target
        .write_all(&[0_u8; HEADER_SIZE])
        .map_err(|error| error.to_string())?;
    let mut entries = Vec::with_capacity(TRACK_COUNT);
    for (index, frames) in track_frames.iter().copied().enumerate() {
        let offset = target
            .stream_position()
            .map_err(|error| error.to_string())? as u32;
        let length = encode_track_pcm(
            source,
            &mut target,
            frames,
            source_frames_per_output as usize,
        )?;
        entries.push((index as u32 + 2, offset, length, frames));
        if index + 1 != TRACK_COUNT {
            source
                .seek(SeekFrom::Current(SEGUE_BYTES as i64))
                .map_err(|error| error.to_string())?;
        }
    }
    finish_container(
        target,
        &temporary,
        output,
        VERSION_PCM,
        sample_rate,
        &entries,
    )
}

/// Decodes one IMA track into raw 16-bit samples.
fn decode_track_into<W: Write>(
    bytes: &[u8],
    offset: usize,
    length: usize,
    frames: u32,
    target: &mut W,
) -> Result<()> {
    let end = offset
        .checked_add(length)
        .ok_or("BGM.dat track offset overflows")?;
    if end > bytes.len() {
        return Err("BGM.dat track lies outside the file".into());
    }
    let mut at = offset;
    let mut left = frames;
    let mut block = Vec::with_capacity(BLOCK_FRAMES * 2);
    while left != 0 {
        if at + 6 > end {
            return Err("BGM.dat block header is truncated".into());
        }
        let count = get_u16(bytes, at) as u32;
        let first = get_u16(bytes, at + 2) as i16;
        let mut step_index = bytes[at + 4] as i32;
        if count == 0
            || count > BLOCK_FRAMES as u32
            || count > left
            || step_index > 88
            || bytes[at + 5] != 0
        {
            return Err("BGM.dat block header is invalid".into());
        }
        let packed = count as usize / 2;
        if at + 6 + packed > end {
            return Err("BGM.dat block is truncated".into());
        }
        let mut predictor = first as i32;
        block.clear();
        block.extend_from_slice(&first.to_le_bytes());
        let mut produced = 1_u32;
        'nibbles: for &byte in &bytes[at + 6..at + 6 + packed] {
            for nibble in [byte & 15, byte >> 4] {
                if produced == count {
                    break 'nibbles;
                }
                let step = STEP_TABLE[step_index as usize];
                let mut difference = step >> 3;
                if nibble & 1 != 0 {
                    difference += step >> 2;
                }
                if nibble & 2 != 0 {
                    difference += step >> 1;
                }
                if nibble & 4 != 0 {
                    difference += step;
                }
                if nibble & 8 != 0 {
                    predictor -= difference;
                } else {
                    predictor += difference;
                }
                predictor = predictor.clamp(i16::MIN as i32, i16::MAX as i32);
                step_index = (step_index + INDEX_TABLE[nibble as usize]).clamp(0, 88);
                block.extend_from_slice(&(predictor as i16).to_le_bytes());
                produced += 1;
            }
        }
        if produced != count {
            return Err("BGM.dat block has too few samples".into());
        }
        target
            .write_all(&block)
            .map_err(|error| format!("write PCM track: {error}"))?;
        at += 6 + packed;
        left -= count;
    }
    Ok(())
}

fn header_tracks(bytes: &[u8]) -> Result<(u32, u32, Vec<(u32, u32, u32, u32)>)> {
    if bytes.len() < HEADER_SIZE || &bytes[..8] != MAGIC {
        return Err("not a Doraemon BGM.dat".into());
    }
    let version = get_u32(bytes, 8);
    let sample_rate = get_u32(bytes, 16);
    if (version != VERSION_IMA && version != VERSION_PCM)
        || get_u32(bytes, 12) != TRACK_COUNT as u32
        || get_u16(bytes, 20) != 1
        || get_u16(bytes, 22) != 16
        || get_u32(bytes, 24) != BLOCK_FRAMES as u32
    {
        return Err("unsupported BGM.dat header".into());
    }
    let mut tracks = Vec::with_capacity(TRACK_COUNT);
    for index in 0..TRACK_COUNT {
        let at = 32 + index * 16;
        tracks.push((
            get_u32(bytes, at),
            get_u32(bytes, at + 4),
            get_u32(bytes, at + 8),
            get_u32(bytes, at + 12),
        ));
    }
    Ok((version, sample_rate, tracks))
}

/// Rewrites an existing BGM.dat as raw 16-bit PCM, decoding version 1 first.
pub fn to_pcm(input: &Path, output: &Path) -> Result<()> {
    if !valid(input) {
        return Err(format!("{} is not a valid BGM.dat", input.display()));
    }
    let bytes = fs::read(input).map_err(|error| format!("{}: {error}", input.display()))?;
    let (version, sample_rate, tracks) = header_tracks(&bytes)?;
    if version == VERSION_PCM {
        return replace_file(input, output);
    }
    let temporary = output.with_extension("dat.dmpatch.tmp");
    let mut target =
        File::create(&temporary).map_err(|error| format!("{}: {error}", temporary.display()))?;
    target
        .write_all(&[0_u8; HEADER_SIZE])
        .map_err(|error| error.to_string())?;
    let mut entries = Vec::with_capacity(TRACK_COUNT);
    for (index, (id, offset, length, frames)) in tracks.iter().copied().enumerate() {
        let at = target
            .stream_position()
            .map_err(|error| error.to_string())? as u32;
        decode_track_into(&bytes, offset as usize, length as usize, frames, &mut target)?;
        let written = frames
            .checked_mul(2)
            .ok_or("BGM.dat track exceeds 4 GiB")?;
        entries.push((id, at, written, frames));
        if id != index as u32 + 2 {
            let _ = fs::remove_file(&temporary);
            return Err("BGM.dat track ids are out of order".into());
        }
    }
    finish_container(
        target,
        &temporary,
        output,
        VERSION_PCM,
        sample_rate,
        &entries,
    )
}

pub fn encode_wav(wav: &Path, output: &Path) -> Result<()> {
    if !crate::cue::valid_wav(wav) {
        return Err("DoraemonMusic.wav is not the verified disc extraction".into());
    }
    let mut source = File::open(wav).map_err(|error| format!("{}: {error}", wav.display()))?;
    source
        .seek(SeekFrom::Start(44))
        .map_err(|error| error.to_string())?;
    encode_reader(&mut source, output, 2)
}

pub fn encode_cue(cue_path: &Path, output: &Path) -> Result<()> {
    let cue = crate::cue::parse(cue_path)?;
    if !crate::cue::valid_cue(cue_path) {
        return Err("CUE/BIN is not the verified Doraemon disc image".into());
    }
    let mut source = File::open(&cue.bin_path)
        .map_err(|error| format!("{}: {error}", cue.bin_path.display()))?;
    source
        .seek(SeekFrom::Start(102_263 * 2_352))
        .map_err(|error| error.to_string())?;
    encode_reader(&mut source, output, 2)
}

pub fn encode_wav_pcm(wav: &Path, output: &Path) -> Result<()> {
    if !crate::cue::valid_wav(wav) {
        return Err("DoraemonMusic.wav is not the verified disc extraction".into());
    }
    let mut source = File::open(wav).map_err(|error| format!("{}: {error}", wav.display()))?;
    source
        .seek(SeekFrom::Start(44))
        .map_err(|error| error.to_string())?;
    encode_reader_pcm(&mut source, output, 2)
}

pub fn encode_cue_pcm(cue_path: &Path, output: &Path) -> Result<()> {
    let cue = crate::cue::parse(cue_path)?;
    if !crate::cue::valid_cue(cue_path) {
        return Err("CUE/BIN is not the verified Doraemon disc image".into());
    }
    let mut source = File::open(&cue.bin_path)
        .map_err(|error| format!("{}: {error}", cue.bin_path.display()))?;
    source
        .seek(SeekFrom::Start(102_263 * 2_352))
        .map_err(|error| error.to_string())?;
    encode_reader_pcm(&mut source, output, 2)
}

fn quality_factor(quality: crate::voice::Compression) -> u32 {
    match quality {
        crate::voice::Compression::Original | crate::voice::Compression::High => 2,
        crate::voice::Compression::Balanced => 3,
        crate::voice::Compression::Compact => 4,
    }
}

pub fn encode_wav_quality(
    wav: &Path,
    output: &Path,
    quality: crate::voice::Compression,
) -> Result<()> {
    if !crate::cue::valid_wav(wav) {
        return Err("DoraemonMusic.wav is not the verified disc extraction".into());
    }
    let mut source = File::open(wav).map_err(|error| format!("{}: {error}", wav.display()))?;
    source
        .seek(SeekFrom::Start(44))
        .map_err(|error| error.to_string())?;
    encode_reader(&mut source, output, quality_factor(quality))
}

pub fn encode_cue_quality(
    cue_path: &Path,
    output: &Path,
    quality: crate::voice::Compression,
) -> Result<()> {
    let cue = crate::cue::parse(cue_path)?;
    if !crate::cue::valid_cue(cue_path) {
        return Err("CUE/BIN is not the verified Doraemon disc image".into());
    }
    let mut source = File::open(&cue.bin_path)
        .map_err(|error| format!("{}: {error}", cue.bin_path.display()))?;
    source
        .seek(SeekFrom::Start(102_263 * 2_352))
        .map_err(|error| error.to_string())?;
    encode_reader(&mut source, output, quality_factor(quality))
}

pub fn valid(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let Ok(size) = file.metadata().map(|metadata| metadata.len()) else {
        return false;
    };
    let mut header = [0_u8; HEADER_SIZE];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    let sample_rate = get_u32(&header, 16);
    let factor = match sample_rate {
        22_050 => 2,
        14_700 => 3,
        11_025 => 4,
        _ => return false,
    };
    if &header[..8] != MAGIC
        || (get_u32(&header, 8) != VERSION_IMA && get_u32(&header, 8) != VERSION_PCM)
        || get_u32(&header, 12) != TRACK_COUNT as u32
        || get_u32(&header, 16) != sample_rate
        || get_u16(&header, 20) != 1
        || get_u16(&header, 22) != 16
        || get_u32(&header, 24) != BLOCK_FRAMES as u32
    {
        return false;
    }
    let raw = get_u32(&header, 8) == VERSION_PCM;
    let mut previous_end = HEADER_SIZE as u64;
    for index in 0..TRACK_COUNT {
        let at = 32 + index * 16;
        let id = get_u32(&header, at);
        let offset = get_u32(&header, at + 4) as u64;
        let length = get_u32(&header, at + 8) as u64;
        let frames = get_u32(&header, at + 12);
        if id != index as u32 + 2
            || offset != previous_end
            || length == 0
            || frames != TRACK_FRAMES[index] * 2 / factor
            || offset + length > size
        {
            return false;
        }
        if raw {
            if length != frames as u64 * 2 {
                return false;
            }
            previous_end = offset + length;
            continue;
        }
        if file.seek(SeekFrom::Start(offset)).is_err() {
            return false;
        }
        let mut encoded_left = length;
        let mut frames_left = frames as u64;
        while frames_left != 0 {
            let expected_frames = frames_left.min(BLOCK_FRAMES as u64);
            let mut block = [0_u8; 6];
            if encoded_left < block.len() as u64
                || file.read_exact(&mut block).is_err()
                || u16::from_le_bytes(block[..2].try_into().unwrap()) as u64 != expected_frames
                || block[4] > 88
                || block[5] != 0
            {
                return false;
            }
            let packed = expected_frames / 2;
            if encoded_left < block.len() as u64 + packed
                || file.seek(SeekFrom::Current(packed as i64)).is_err()
            {
                return false;
            }
            encoded_left -= block.len() as u64 + packed;
            frames_left -= expected_frames;
        }
        if encoded_left != 0 {
            return false;
        }
        previous_end = offset + length;
    }
    previous_end == size
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_valid_file(path: &Path) {
        let mut bytes = vec![0_u8; HEADER_SIZE];
        bytes[..8].copy_from_slice(MAGIC);
        put_u32(&mut bytes, 8, 1);
        put_u32(&mut bytes, 12, TRACK_COUNT as u32);
        put_u32(&mut bytes, 16, SAMPLE_RATE);
        put_u16(&mut bytes, 20, 1);
        put_u16(&mut bytes, 22, 16);
        put_u32(&mut bytes, 24, BLOCK_FRAMES as u32);
        for (index, frames) in TRACK_FRAMES.into_iter().enumerate() {
            let offset = bytes.len();
            let mut left = frames as usize;
            while left != 0 {
                let count = left.min(BLOCK_FRAMES);
                bytes.extend_from_slice(&(count as u16).to_le_bytes());
                bytes.extend_from_slice(&0_i16.to_le_bytes());
                bytes.extend_from_slice(&[0, 0]);
                bytes.resize(bytes.len() + count / 2, 0);
                left -= count;
            }
            let at = 32 + index * 16;
            let length = bytes.len() - offset;
            put_u32(&mut bytes, at, index as u32 + 2);
            put_u32(&mut bytes, at + 4, offset as u32);
            put_u32(&mut bytes, at + 8, length as u32);
            put_u32(&mut bytes, at + 12, frames);
        }
        std::fs::write(path, bytes).unwrap();
    }

    fn synthetic_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "doraemon-bgm-{label}-{}-{}.dat",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    #[test]
    fn ima_encoder_is_deterministic() {
        let mut predictor = 0;
        let mut index = 0;
        let values: Vec<_> = [1000_i16, 2000, -1000, 0]
            .into_iter()
            .map(|sample| encode_nibble(sample, &mut predictor, &mut index))
            .collect();
        assert_eq!(values, vec![7, 7, 15, 1]);
        assert!((0..=88).contains(&index));
    }

    #[test]
    fn stereo_pairs_downsample_deterministically() {
        let mut bytes = Vec::new();
        for sample in [1000_i16, -1000, 3000, 1000] {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        assert_eq!(mono_sample(&bytes), 1000);
    }

    #[test]
    fn pcm_decode_reconstructs_the_encoder_predictor() {
        let first = 1000_i16;
        let mut predictor = first as i32;
        let mut step_index = 0_i32;
        let mut expected = vec![first];
        let mut nibbles = Vec::new();
        for sample in [1200_i16, 1300, 900, -400, 0, 32767, -32768, 1234] {
            nibbles.push(encode_nibble(sample, &mut predictor, &mut step_index));
            expected.push(predictor as i16);
        }
        let mut block = Vec::new();
        block.extend_from_slice(&((expected.len() as u16).to_le_bytes()));
        block.extend_from_slice(&first.to_le_bytes());
        block.push(0);
        block.push(0);
        for pair in nibbles.chunks(2) {
            block.push(pair[0] | (pair.get(1).copied().unwrap_or(0) << 4));
        }
        let mut decoded = Vec::new();
        decode_track_into(&block, 0, block.len(), expected.len() as u32, &mut decoded).unwrap();
        let samples: Vec<i16> = decoded
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        assert_eq!(samples, expected);
    }

    #[test]
    fn pcm_container_validates_and_reports_bad_lengths() {
        let path = synthetic_path("pcm-container");
        let mut entries = Vec::new();
        let mut bytes = vec![0_u8; HEADER_SIZE];
        for (index, frames) in TRACK_FRAMES.into_iter().enumerate() {
            let offset = bytes.len() as u32;
            bytes.resize(bytes.len() + frames as usize * 2, index as u8);
            entries.push((index as u32 + 2, offset, frames * 2, frames));
        }
        let mut header = [0_u8; HEADER_SIZE];
        write_header(&mut header, VERSION_PCM, SAMPLE_RATE, &entries);
        bytes[..HEADER_SIZE].copy_from_slice(&header);
        std::fs::write(&path, &bytes).unwrap();
        assert!(valid(&path));

        put_u32(&mut bytes, 32 + 8, TRACK_FRAMES[0] * 2 + 2);
        let damaged = synthetic_path("pcm-length");
        std::fs::write(&damaged, &bytes).unwrap();
        assert!(!valid(&damaged));
        std::fs::remove_file(damaged).unwrap();
        std::fs::remove_file(path).unwrap();
    }


    #[test]
    fn all_track_durations_match_the_corrected_music() {
        let seconds: Vec<_> = TRACK_FRAMES
            .into_iter()
            .map(|frames| (frames + SAMPLE_RATE - 1) / SAMPLE_RATE)
            .collect();
        assert_eq!(seconds, [338, 128, 40, 107, 6, 95, 68, 61, 85, 13]);
    }

    #[test]
    fn validates_all_tracks_and_rejects_corruption() {
        let path = synthetic_path("validation");
        synthetic_valid_file(&path);
        assert!(valid(&path));

        let original = std::fs::read(&path).unwrap();
        for (label, offset, value) in [
            ("magic", 0, b'X'),
            ("version", 8, 3),
            ("track-id", 32, 3),
            ("frame-count", 44, 0),
        ] {
            let damaged = synthetic_path(label);
            let mut bytes = original.clone();
            bytes[offset] = value;
            std::fs::write(&damaged, bytes).unwrap();
            assert!(!valid(&damaged), "accepted corrupted {label}");
            std::fs::remove_file(damaged).unwrap();
        }

        let first_block = HEADER_SIZE;
        let damaged = synthetic_path("index");
        let mut bytes = original.clone();
        bytes[first_block + 4] = 89;
        std::fs::write(&damaged, bytes).unwrap();
        assert!(!valid(&damaged));
        std::fs::remove_file(damaged).unwrap();

        let damaged = synthetic_path("truncated");
        std::fs::write(&damaged, &original[..original.len() - 1]).unwrap();
        assert!(!valid(&damaged));
        std::fs::remove_file(damaged).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn real_disc_encodes_when_fixture_is_available() {
        let Ok(cue) = std::env::var("DORAEMON_TEST_CUE") else {
            return;
        };
        let output = synthetic_path("cue-fixture");
        let compact_output = synthetic_path("compact-cue-fixture");
        let wav = synthetic_path("fixture").with_extension("wav");
        let wav_output = synthetic_path("wav-fixture");
        for path in [&output, &compact_output, &wav, &wav_output] {
            let _ = std::fs::remove_file(path);
        }
        encode_cue(Path::new(&cue), &output).unwrap();
        encode_cue_quality(
            Path::new(&cue),
            &compact_output,
            crate::voice::Compression::Compact,
        )
        .unwrap();
        crate::cue::extract(Path::new(&cue), &wav).unwrap();
        encode_wav(&wav, &wav_output).unwrap();
        assert!(valid(&output) && valid(&wav_output));
        assert_eq!(std::fs::read(&output).unwrap(), std::fs::read(&wav_output).unwrap());
        let size = std::fs::metadata(&output).unwrap().len();
        assert!((8_000_000..15_000_000).contains(&size));
        let compact_size = std::fs::metadata(&compact_output).unwrap().len();
        eprintln!("Compact BGM.dat: {compact_size} bytes from {size} bytes");
        assert!(valid(&compact_output));
        assert!(compact_size < 8_000_000);
        std::fs::remove_file(output).unwrap();
        std::fs::remove_file(compact_output).unwrap();
        std::fs::remove_file(wav_output).unwrap();
        std::fs::remove_file(wav).unwrap();
    }
}

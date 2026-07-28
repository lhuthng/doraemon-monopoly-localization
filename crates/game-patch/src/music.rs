use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::{install::replace_file, Result};

pub const MAGIC: &[u8; 8] = b"DBGM1\0\0\0";
pub const HEADER_SIZE: usize = 192;
pub const TRACK_COUNT: usize = 10;
pub const SAMPLE_RATE: u32 = 22_050;
pub const BLOCK_FRAMES: usize = 4_096;
pub const TRACK_FRAMES: [u32; TRACK_COUNT] = [
    7_525_812, 2_865_912, 924_924, 2_394_924, 174_048, 2_130_324, 1_541_736, 1_376_802, 1_903_944,
    308_994,
];

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

fn encode_reader<R: Read>(
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
        || get_u32(&header, 8) != 1
        || get_u32(&header, 12) != TRACK_COUNT as u32
        || get_u32(&header, 16) != sample_rate
        || get_u16(&header, 20) != 1
        || get_u16(&header, 22) != 16
        || get_u32(&header, 24) != BLOCK_FRAMES as u32
    {
        return false;
    }
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
    fn all_track_durations_match_the_verified_disc() {
        let seconds: Vec<_> = TRACK_FRAMES
            .into_iter()
            .map(|frames| (frames + SAMPLE_RATE - 1) / SAMPLE_RATE)
            .collect();
        assert_eq!(seconds, [342, 130, 42, 109, 8, 97, 70, 63, 87, 15]);
    }

    #[test]
    fn validates_all_tracks_and_rejects_corruption() {
        let path = synthetic_path("validation");
        synthetic_valid_file(&path);
        assert!(valid(&path));

        let original = std::fs::read(&path).unwrap();
        for (label, offset, value) in [
            ("magic", 0, b'X'),
            ("version", 8, 2),
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

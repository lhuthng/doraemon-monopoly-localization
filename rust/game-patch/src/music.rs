use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::{install::replace_file, Result};

pub const MAGIC: &[u8; 8] = b"DMUSIC1\0";
pub const HEADER_SIZE: usize = 192;
pub const TRACK_COUNT: usize = 10;
pub const SAMPLE_RATE: u32 = 44_100;
pub const BLOCK_FRAMES: usize = 4_096;
pub const TRACK_FRAMES: [u32; TRACK_COUNT] = [
    15_051_624, 5_731_824, 1_849_848, 4_789_848, 348_096, 4_260_648, 3_083_472, 2_753_604,
    3_807_888, 617_988,
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

fn encode_track<R: Read>(source: &mut R, target: &mut File, frames: u32) -> Result<u32> {
    let mut remaining = frames as usize;
    let mut written = 0_u32;
    while remaining != 0 {
        let count = remaining.min(BLOCK_FRAMES);
        let mut pcm = vec![0_u8; count * 4];
        source
            .read_exact(&mut pcm)
            .map_err(|error| format!("read source PCM: {error}"))?;
        let first_l = i16::from_le_bytes(pcm[0..2].try_into().unwrap());
        let first_r = i16::from_le_bytes(pcm[2..4].try_into().unwrap());
        let mut block = Vec::with_capacity(count + 9);
        block.extend_from_slice(&(count as u16).to_le_bytes());
        block.extend_from_slice(&first_l.to_le_bytes());
        block.extend_from_slice(&[0, 0]);
        block.extend_from_slice(&first_r.to_le_bytes());
        block.extend_from_slice(&[0, 0]);
        let mut predictor_l = first_l as i32;
        let mut predictor_r = first_r as i32;
        let mut index_l = 0;
        let mut index_r = 0;
        for frame in 1..count {
            let offset = frame * 4;
            let left = i16::from_le_bytes(pcm[offset..offset + 2].try_into().unwrap());
            let right = i16::from_le_bytes(pcm[offset + 2..offset + 4].try_into().unwrap());
            let packed = encode_nibble(left, &mut predictor_l, &mut index_l)
                | (encode_nibble(right, &mut predictor_r, &mut index_r) << 4);
            block.push(packed);
        }
        target
            .write_all(&block)
            .map_err(|error| format!("write Music.dat block: {error}"))?;
        written = written
            .checked_add(block.len() as u32)
            .ok_or("Music.dat track exceeds 4 GiB")?;
        remaining -= count;
    }
    Ok(written)
}

fn encode_reader<R: Read>(source: &mut R, output: &Path, track_frames: &[u32]) -> Result<()> {
    if track_frames.len() != TRACK_COUNT {
        return Err("Music.dat requires exactly ten tracks".into());
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
        let length = encode_track(source, &mut target, frames)?;
        entries.push((index as u32 + 2, offset, length, frames));
    }
    let mut header = [0_u8; HEADER_SIZE];
    header[..8].copy_from_slice(MAGIC);
    put_u32(&mut header, 8, 1);
    put_u32(&mut header, 12, TRACK_COUNT as u32);
    put_u32(&mut header, 16, SAMPLE_RATE);
    put_u16(&mut header, 20, 2);
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
        return Err("generated Music.dat failed verification".into());
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
    encode_reader(&mut source, output, &TRACK_FRAMES)
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
    encode_reader(&mut source, output, &TRACK_FRAMES)
}

pub fn valid(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let Ok(size) = file.metadata().map(|metadata| metadata.len()) else {
        return false;
    };
    let mut header = [0_u8; HEADER_SIZE];
    if file.read_exact(&mut header).is_err()
        || &header[..8] != MAGIC
        || get_u32(&header, 8) != 1
        || get_u32(&header, 12) != TRACK_COUNT as u32
        || get_u32(&header, 16) != SAMPLE_RATE
        || get_u16(&header, 20) != 2
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
            || frames != TRACK_FRAMES[index]
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
            let mut block = [0_u8; 10];
            if encoded_left < block.len() as u64
                || file.read_exact(&mut block).is_err()
                || u16::from_le_bytes(block[..2].try_into().unwrap()) as u64 != expected_frames
                || block[4] > 88
                || block[5] != 0
                || block[8] > 88
                || block[9] != 0
            {
                return false;
            }
            let samples = expected_frames - 1;
            if encoded_left < block.len() as u64 + samples
                || file.seek(SeekFrom::Current(samples as i64)).is_err()
            {
                return false;
            }
            encoded_left -= block.len() as u64 + samples;
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
    fn real_disc_encodes_when_fixture_is_available() {
        let Ok(cue) = std::env::var("DORAEMON_TEST_CUE") else {
            return;
        };
        let output =
            std::env::temp_dir().join(format!("doraemon-music-test-{}.dat", std::process::id()));
        let _ = std::fs::remove_file(&output);
        encode_cue(Path::new(&cue), &output).unwrap();
        assert!(valid(&output));
        let size = std::fs::metadata(&output).unwrap().len();
        assert!((35_000_000..50_000_000).contains(&size));
        std::fs::remove_file(output).unwrap();
    }
}

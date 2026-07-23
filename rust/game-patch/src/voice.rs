use crate::{hash, payload::VoicePatch, strings, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compression {
    Original,
    High,
    Balanced,
    Compact,
}
impl Default for Compression { fn default() -> Self { Self::Original } }

fn le16(data: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(data.get(at..at + 2).ok_or("truncated WAV")?.try_into().unwrap()))
}
fn le32(data: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(data.get(at..at + 4).ok_or("truncated WAV")?.try_into().unwrap()))
}

/// Rebuilds the archive with only standard PCM WAV leaves changed.  The game
/// already passes these leaves to DirectSound, so no executable-side decoder is
/// required.  Unsupported WAV layouts are deliberately retained unchanged.
pub fn compress_audio(source: &[u8], quality: Compression) -> Result<Vec<u8>> {
    let records = strings::packed_records(source)?;
    let mut replacements = std::collections::BTreeMap::new();
    for (id, record) in records {
        let decoded = if record.starts_with(b"RIFF") && record.get(8..12) == Some(b"WAVE") {
            record.clone()
        } else {
            match strings::decompress(&record) {
                Ok(decoded)
                    if decoded.starts_with(b"RIFF")
                        && decoded.get(8..12) == Some(b"WAVE") =>
                {
                    decoded
                }
                _ => continue,
            }
        };
        let candidate = transcode_wav(&decoded, quality)?;
        let packed = strings::compress(&candidate)?;
        let replacement = if packed.len() < candidate.len() {
            packed
        } else {
            candidate
        };
        if replacement != record {
            replacements.insert(id, replacement);
        }
    }
    if replacements.is_empty() { return Ok(source.to_vec()); }
    strings::rebuild_packed(source, &replacements)
}

fn transcode_wav(wav: &[u8], quality: Compression) -> Result<Vec<u8>> {
    if wav.len() < 44 || wav.get(12..16) != Some(b"fmt ") || le32(wav, 16)? != 16
        || le16(wav, 20)? != 1 || le16(wav, 22)? != 1 || le16(wav, 34)? != 16
        || wav.get(36..40) != Some(b"data") { return Ok(wav.to_vec()); }
    let rate = le32(wav, 24)?;
    if rate != 22_050 { return Ok(wav.to_vec()); }
    let length = le32(wav, 40)? as usize;
    if 44 + length > wav.len() || length % 2 != 0 { return Ok(wav.to_vec()); }
    let (out_rate, bits, stride): (u32, u16, usize) = match quality {
        Compression::Original => (22_050, 16, 1),
        Compression::High => (22_050, 8, 1),
        Compression::Balanced => (11_025, 16, 2),
        Compression::Compact => (11_025, 8, 2),
    };
    if quality == Compression::Original { return Ok(wav.to_vec()); }
    let samples: Vec<i16> = wav[44..44 + length].chunks_exact(2).map(|v| i16::from_le_bytes([v[0], v[1]])).collect();
    let mut pcm = Vec::with_capacity(samples.len() / stride * (bits / 8) as usize);
    for group in samples.chunks(stride) {
        let value = (group.iter().map(|v| *v as i32).sum::<i32>() / group.len() as i32) as i16;
        if bits == 8 { pcm.push(((value as i32 + 32768) >> 8) as u8); }
        else { pcm.extend_from_slice(&value.to_le_bytes()); }
    }
    let mut output = vec![0; 44];
    output[..4].copy_from_slice(b"RIFF"); output[8..12].copy_from_slice(b"WAVE"); output[12..16].copy_from_slice(b"fmt ");
    output[16..20].copy_from_slice(&16u32.to_le_bytes()); output[20..22].copy_from_slice(&1u16.to_le_bytes()); output[22..24].copy_from_slice(&1u16.to_le_bytes());
    output[24..28].copy_from_slice(&out_rate.to_le_bytes()); let block = (bits / 8) as u16;
    output[28..32].copy_from_slice(&(out_rate * block as u32).to_le_bytes()); output[32..34].copy_from_slice(&block.to_le_bytes()); output[34..36].copy_from_slice(&bits.to_le_bytes()); output[36..40].copy_from_slice(b"data"); output[40..44].copy_from_slice(&(pcm.len() as u32).to_le_bytes()); output.extend_from_slice(&pcm); let riff_size = (output.len() - 8) as u32; output[4..8].copy_from_slice(&riff_size.to_le_bytes());
    Ok(output)
}

pub fn create_patch(base: &[u8], target: &[u8]) -> Result<VoicePatch> {
    let base_records = strings::packed_records(base)?;
    let target_records = strings::packed_records(target)?;
    if base_records.keys().collect::<Vec<_>>() != target_records.keys().collect::<Vec<_>>() {
        return Err("source and localized voice.dat have different record IDs".into());
    }
    let replacements = target_records
        .into_iter()
        .filter(|(id, packed)| base_records.get(id) != Some(packed))
        .collect();
    Ok(VoicePatch {
        expected_ids: base_records.keys().cloned().collect(),
        replacements,
        base_hash: hash::bytes(base),
        target_hash: hash::bytes(target),
        base_len: base.len() as u64,
        target_len: target.len() as u64,
    })
}

pub fn apply_patch(source: &[u8], patch: &VoicePatch) -> Result<Vec<u8>> {
    let digest = hash::bytes(source);
    if digest == patch.target_hash && source.len() as u64 == patch.target_len {
        return Ok(source.to_vec());
    }
    if digest != patch.base_hash || source.len() as u64 != patch.base_len {
        return Err("voice.dat does not match the supported original archive".into());
    }
    let records = strings::packed_records(source)?;
    if records.keys().cloned().collect::<Vec<_>>() != patch.expected_ids {
        return Err("voice.dat record layout does not match this release".into());
    }
    let rebuilt = strings::rebuild_packed(source, &patch.replacements)?;
    if rebuilt.len() as u64 != patch.target_len || hash::bytes(&rebuilt) != patch.target_hash {
        return Err("voice.dat failed rebuilt verification".into());
    }
    Ok(rebuilt)
}

pub fn matches(source: &[u8], patch: &VoicePatch) -> bool {
    source.len() as u64 == patch.target_len && hash::bytes(source) == patch.target_hash
}

pub fn replacement_count(patch: &VoicePatch) -> usize {
    patch.replacements.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_voice_compact_reduces_all_wav_leaves_when_fixture_is_supplied() {
        let Ok(path) = std::env::var("DORAEMON_TEST_VOICE_COMPRESSION") else {
            return;
        };
        let source = std::fs::read(path).unwrap();
        let compact = compress_audio(&source, Compression::Compact).unwrap();
        eprintln!(
            "Compact Voice.dat: {} bytes from {} bytes",
            compact.len(),
            source.len()
        );
        assert!(
            compact.len() < source.len() / 2,
            "Compact Voice.dat was {} bytes from {} bytes",
            compact.len(),
            source.len()
        );
    }

    #[test]
    fn real_voice_patch_round_trip_when_fixtures_are_supplied() {
        let (Ok(base), Ok(target)) = (
            std::env::var("DORAEMON_TEST_VOICE_BASE"),
            std::env::var("DORAEMON_TEST_VOICE_TARGET"),
        ) else {
            return;
        };
        let base = std::fs::read(base).unwrap();
        let target = std::fs::read(target).unwrap();
        let patch = create_patch(&base, &target).unwrap();
        let rebuilt = apply_patch(&base, &patch).unwrap();
        assert_eq!(rebuilt, target);
        assert!(matches(&rebuilt, &patch));
    }
}

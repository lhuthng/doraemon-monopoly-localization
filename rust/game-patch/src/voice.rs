use crate::{hash, payload::VoicePatch, strings, Result};

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

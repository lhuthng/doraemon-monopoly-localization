use std::collections::{BTreeMap, BTreeSet};

use crate::{delta, hash::Hash, Result};

const MAGIC: &[u8; 8] = b"DMPATCH5";
const LEGACY_MAGIC: &[u8; 8] = b"DMPATCH4";
const PART_MAGIC: &[u8; 4] = b"DMPT";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    English,
    Vietnamese,
    Custom,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Vietnamese => "Vietnamese",
            Self::Custom => "Portable compatibility",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetName {
    Doraemon,
    Nobita,
    Dorami,
    Shizuka,
    Suneo,
    Gian,
    Others,
    Sprites,
    Runtime,
}

impl TargetName {
    pub fn all() -> &'static [TargetName] {
        &[
            Self::Doraemon,
            Self::Nobita,
            Self::Dorami,
            Self::Shizuka,
            Self::Suneo,
            Self::Gian,
            Self::Others,
            Self::Sprites,
            Self::Runtime,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Doraemon => "doraemon",
            Self::Nobita => "nobita",
            Self::Dorami => "dorami",
            Self::Shizuka => "shizuka",
            Self::Suneo => "suneo",
            Self::Gian => "gian",
            Self::Others => "others",
            Self::Sprites => "sprites",
            Self::Runtime => "runtime",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "doraemon" => Some(Self::Doraemon),
            "nobita" => Some(Self::Nobita),
            "dorami" => Some(Self::Dorami),
            "shizuka" => Some(Self::Shizuka),
            "suneo" => Some(Self::Suneo),
            "gian" => Some(Self::Gian),
            "others" => Some(Self::Others),
            "sprites" => Some(Self::Sprites),
            "runtime" => Some(Self::Runtime),
            _ => None,
        }
    }

    pub fn filename(self) -> String {
        match self {
            Self::Doraemon => "loc-doraemon.dmpatch".into(),
            Self::Nobita => "loc-nobita.dmpatch".into(),
            Self::Dorami => "loc-dorami.dmpatch".into(),
            Self::Shizuka => "loc-shizuka.dmpatch".into(),
            Self::Suneo => "loc-suneo.dmpatch".into(),
            Self::Gian => "loc-gian.dmpatch".into(),
            Self::Others => "loc-others.dmpatch".into(),
            Self::Sprites => "sprites.dmpatch".into(),
            Self::Runtime => "runtime.dmpatch".into(),
        }
    }

    pub fn target_type(self) -> TargetType {
        match self {
            Self::Doraemon
            | Self::Nobita
            | Self::Dorami
            | Self::Shizuka
            | Self::Suneo
            | Self::Gian => TargetType::Character,
            Self::Others => TargetType::Others,
            Self::Sprites => TargetType::Sprites,
            Self::Runtime => TargetType::Runtime,
        }
    }

    /// String groups owned by this target.
    pub fn string_groups(self) -> &'static [&'static str] {
        match self {
            Self::Doraemon => &["003"],
            Self::Nobita => &["004"],
            Self::Dorami => &["005"],
            Self::Shizuka => &["006"],
            Self::Suneo => &["007"],
            Self::Gian => &["008"],
            Self::Others => &["000", "001", "002"],
            Self::Sprites | Self::Runtime => &[],
        }
    }

    /// Character index in voice.dat (0-based), if applicable.
    pub fn character_index(self) -> Option<u16> {
        match self {
            Self::Doraemon => Some(0),
            Self::Nobita => Some(1),
            Self::Dorami => Some(2),
            Self::Shizuka => Some(3),
            Self::Suneo => Some(4),
            Self::Gian => Some(5),
            Self::Others | Self::Sprites | Self::Runtime => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetType {
    Character,
    Others,
    Sprites,
    Runtime,
}

impl TargetType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Others => "others",
            Self::Sprites => "sprites",
            Self::Runtime => "runtime",
        }
    }
}

/// A string record ID starts with a three-digit group number. Returns the
/// group portion.
pub fn string_record_group(id: &str) -> &str {
    id.get(..3).unwrap_or("")
}

/// Returns true when `id` is a voice record that belongs to `character_index`.
/// Voice paths are `character/bank/slot` — the first component is the character.
pub fn voice_record_character(id: &str) -> Option<u16> {
    id.split('/').next().and_then(|part| part.parse::<u16>().ok())
}

/// Shared action text records 000/031 through 000/035.
#[allow(dead_code)]
const SHARED_ACTION_TEXT_IDS: &[&str] = &[
    "000/031", "000/032", "000/033", "000/034", "000/035",
];

/// Shared action voice records 00*/001/011 through 00*/001/015 (all 6 characters).
pub fn is_shared_action_voice(id: &str) -> bool {
    let parts: Vec<&str> = id.split('/').collect();
    parts.len() == 3 && parts[1] == "001" && {
        parts[2].parse::<u16>().ok()
            .is_some_and(|slot| (11..=15).contains(&slot))
    }
}

/// Determine whether a string record belongs to a given target.
pub fn string_belongs_to(id: &str, target: TargetName) -> bool {
    let group = string_record_group(id);
    target.string_groups().contains(&group)
}

/// Determine whether a voice record belongs to a given target.
pub fn voice_belongs_to(id: &str, target: TargetName) -> bool {
    if let Some(char_idx) = target.character_index() {
        voice_record_character(id) == Some(char_idx)
    } else if target == TargetName::Others {
        // Others owns: menu/misc voice records not owned by any character.
        let ch = voice_record_character(id);
        ch.is_none() || ch.unwrap_or(6) >= 6
            || is_shared_action_voice(id)
    } else {
        false
    }
}

/// Filters a StringsPatch to only include records belonging to a target.
pub fn filter_strings(patch: &StringsPatch, target: TargetName) -> StringsPatch {
    let replacements: BTreeMap<_, _> = patch
        .replacements
        .iter()
        .filter(|(id, _)| string_belongs_to(id, target))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let expected_ids: Vec<_> = patch
        .expected_ids
        .iter()
        .filter(|id| string_belongs_to(id, target))
        .cloned()
        .collect();
    StringsPatch {
        expected_ids,
        replacements,
    }
}

/// Filters a VoicePatch to only include records belonging to a target.
pub fn filter_voice(patch: &VoicePatch, target: TargetName) -> VoicePatch {
    let replacements: BTreeMap<_, _> = patch
        .replacements
        .iter()
        .filter(|(id, _)| voice_belongs_to(id, target))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let expected_ids: Vec<_> = patch
        .expected_ids
        .iter()
        .filter(|id| voice_belongs_to(id, target))
        .cloned()
        .collect();
    VoicePatch {
        expected_ids,
        replacements,
        base_hash: patch.base_hash,
        target_hash: patch.target_hash,
        base_len: patch.base_len,
        target_len: patch.target_len,
    }
}

#[derive(Clone, Debug)]
pub struct FilePatch {
    pub name: String,
    pub base_hash: Hash,
    pub target_hash: Hash,
    pub base_len: u64,
    pub target_len: u64,
    pub delta: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct RequiredFile {
    pub name: String,
    pub hash: Hash,
    pub len: u64,
}

#[derive(Clone, Debug)]
pub struct BundledFile {
    pub name: String,
    pub hash: Hash,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct StringsPatch {
    pub expected_ids: Vec<String>,
    pub replacements: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Default)]
pub struct VoicePatch {
    pub expected_ids: Vec<String>,
    pub replacements: BTreeMap<String, Vec<u8>>,
    pub base_hash: Hash,
    pub target_hash: Hash,
    pub base_len: u64,
    pub target_len: u64,
}

impl FilePatch {
    pub fn create(name: impl Into<String>, source: &[u8], target: &[u8]) -> Result<Self> {
        Ok(Self {
            name: name.into(),
            base_hash: crate::hash::bytes(source),
            target_hash: crate::hash::bytes(target),
            base_len: source.len() as u64,
            target_len: target.len() as u64,
            delta: delta::build(source, target)?,
        })
    }

    pub fn apply(&self, source: &[u8]) -> Result<Vec<u8>> {
        if source.len() as u64 != self.base_len || crate::hash::bytes(source) != self.base_hash {
            return Err(format!(
                "{} does not match the supported Cantonese release",
                self.name
            ));
        }
        let output = delta::apply(source, &self.delta)?;
        if output.len() as u64 != self.target_len || crate::hash::bytes(&output) != self.target_hash
        {
            return Err(format!("{} failed target verification", self.name));
        }
        Ok(output)
    }
}

#[derive(Clone, Debug)]
pub struct PatchProfile {
    pub name: String,
    pub required: Vec<RequiredFile>,
    pub files: Vec<FilePatch>,
    pub executable_plain: Option<FilePatch>,
    pub executable_portable: FilePatch,
}

#[derive(Clone, Debug)]
pub struct Payload {
    pub language: Language,
    pub profiles: Vec<PatchProfile>,
    pub strings: Option<StringsPatch>,
    pub voice: Option<VoicePatch>,
    pub bundled: Vec<BundledFile>,
}

/// One part of the multipart payload format. Nine parts together form a
/// complete language patch.
#[derive(Clone, Debug)]
pub struct PayloadPart {
    pub format_version: u16,
    pub language: Language,
    pub target: TargetName,
    pub base_fingerprints: Vec<RequiredFile>,
    pub strings: Option<StringsPatch>,
    pub voice: Option<VoicePatch>,
    pub file_patches: Vec<FilePatch>,
    pub executable_plain: Option<FilePatch>,
    pub executable_portable: Option<FilePatch>,
    pub bundled: Vec<BundledFile>,
    pub is_empty: bool,
}

/// Merge multiple PayloadParts into a single unified Payload for installation.
/// Validates that all parts have the same language and no conflicting records.
pub fn merge_parts(parts: &[PayloadPart]) -> Result<Payload> {
    if parts.is_empty() {
        return Err("cannot merge an empty part set".into());
    }
    let language = parts[0].language;
    let mut all_strings: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut all_expected_strings: BTreeSet<String> = BTreeSet::new();
    let mut all_voice: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut all_expected_voice: BTreeSet<String> = BTreeSet::new();
    let mut all_file_patches: Vec<FilePatch> = Vec::new();
    let mut executable_plain: Option<FilePatch> = None;
    let mut executable_portable: Option<FilePatch> = None;
    let mut all_bundled: Vec<BundledFile> = Vec::new();
    let mut profiles: Vec<PatchProfile> = Vec::new();

    for part in parts {
        if part.language != language {
            return Err(format!(
                "part {} has language {:?}, expected {:?}",
                part.target.label(),
                part.language as u8,
                language as u8,
            ));
        }
        if let Some(strings) = &part.strings {
            for id in &strings.expected_ids {
                if !all_expected_strings.insert(id.clone()) {
                    return Err(format!(
                        "string record {id} is owned by multiple targets"
                    ));
                }
            }
            for (id, value) in &strings.replacements {
                if all_strings.insert(id.clone(), value.clone()).is_some() {
                    return Err(format!(
                        "string replacement {id} is provided by multiple targets"
                    ));
                }
            }
        }
        if let Some(voice) = &part.voice {
            for id in &voice.expected_ids {
                if !all_expected_voice.insert(id.clone()) {
                    return Err(format!(
                        "voice record {id} is owned by multiple targets"
                    ));
                }
            }
            for (id, packed) in &voice.replacements {
                if all_voice.insert(id.clone(), packed.clone()).is_some() {
                    return Err(format!(
                        "voice replacement {id} is provided by multiple targets"
                    ));
                }
            }
        }
        for fp in &part.file_patches {
            if !all_file_patches.iter().any(|p| p.name == fp.name) {
                all_file_patches.push(fp.clone());
            }
        }
        if part.executable_plain.is_some() {
            executable_plain = part.executable_plain.clone();
        }
        if part.executable_portable.is_some() {
            executable_portable = part.executable_portable.clone();
        }
        for bf in &part.bundled {
            if !all_bundled.iter().any(|b| b.name == bf.name) {
                all_bundled.push(bf.clone());
            }
        }
    }

    let strings_patch = if all_strings.is_empty() {
        None
    } else {
        Some(StringsPatch {
            expected_ids: all_expected_strings.into_iter().collect(),
            replacements: all_strings,
        })
    };

    let voice_patch = if all_voice.is_empty() {
        None
    } else {
        Some(VoicePatch {
            expected_ids: all_expected_voice.into_iter().collect(),
            replacements: all_voice,
            // Use the first available voice patch's hashes
            base_hash: parts
                .iter()
                .find_map(|p| p.voice.as_ref().map(|v| v.base_hash))
                .unwrap_or([0; 32]),
            target_hash: parts
                .iter()
                .find_map(|p| p.voice.as_ref().map(|v| v.target_hash))
                .unwrap_or([0; 32]),
            base_len: parts
                .iter()
                .find_map(|p| p.voice.as_ref().map(|v| v.base_len))
                .unwrap_or(0),
            target_len: parts
                .iter()
                .find_map(|p| p.voice.as_ref().map(|v| v.target_len))
                .unwrap_or(0),
        })
    };

    let profile = PatchProfile {
        name: format!("{} merged parts", language.label()),
        required: parts
            .iter()
            .flat_map(|p| p.base_fingerprints.iter().cloned())
            .collect(),
        files: all_file_patches,
        executable_plain,
        executable_portable: executable_portable.unwrap_or_else(|| {
            FilePatch {
                name: "Doraemon.exe".into(),
                base_hash: [0; 32],
                target_hash: [0; 32],
                base_len: 0,
                target_len: 0,
                delta: Vec::new(),
            }
        }),
    };
    profiles.push(profile);

    Ok(Payload {
        language,
        profiles,
        strings: strings_patch,
        voice: voice_patch,
        bundled: all_bundled,
    })
}

pub fn encode_part(part: &PayloadPart) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    output.extend_from_slice(PART_MAGIC);
    output.extend_from_slice(&part.format_version.to_le_bytes());
    output.push(part.target as u8);
    output.push(part.target.target_type() as u8);
    output.push(match part.language {
        Language::English => 0,
        Language::Vietnamese => 1,
        Language::Custom => 2,
    });
    output.push(part.is_empty as u8);
    // reserved
    output.extend_from_slice(&[0u8; 4]);
    // base fingerprints
    output.extend_from_slice(&(part.base_fingerprints.len() as u16).to_le_bytes());
    for file in &part.base_fingerprints {
        encode_required(&mut output, file)?;
    }
    // file patches
    output.extend_from_slice(&(part.file_patches.len() as u16).to_le_bytes());
    for fp in &part.file_patches {
        encode_file(&mut output, fp)?;
    }
    // strings
    output.push(part.strings.is_some() as u8);
    if let Some(strings) = &part.strings {
        output.extend_from_slice(&(strings.expected_ids.len() as u32).to_le_bytes());
        for id in &strings.expected_ids {
            put_bytes(&mut output, id.as_bytes())?;
        }
        output.extend_from_slice(&(strings.replacements.len() as u32).to_le_bytes());
        for (id, value) in &strings.replacements {
            put_bytes(&mut output, id.as_bytes())?;
            put_bytes(&mut output, value)?;
        }
    }
    // voice
    output.push(part.voice.is_some() as u8);
    if let Some(voice) = &part.voice {
        output.extend_from_slice(&voice.base_hash);
        output.extend_from_slice(&voice.target_hash);
        output.extend_from_slice(&voice.base_len.to_le_bytes());
        output.extend_from_slice(&voice.target_len.to_le_bytes());
        output.extend_from_slice(&(voice.expected_ids.len() as u32).to_le_bytes());
        for id in &voice.expected_ids {
            put_bytes(&mut output, id.as_bytes())?;
        }
        output.extend_from_slice(&(voice.replacements.len() as u32).to_le_bytes());
        for (id, packed) in &voice.replacements {
            put_bytes(&mut output, id.as_bytes())?;
            put_bytes(&mut output, packed)?;
        }
    }
    // executable
    output.push(part.executable_plain.is_some() as u8);
    if let Some(fp) = &part.executable_plain {
        encode_file(&mut output, fp)?;
    }
    output.push(part.executable_portable.is_some() as u8);
    if let Some(fp) = &part.executable_portable {
        encode_file(&mut output, fp)?;
    }
    // bundled
    output.extend_from_slice(&(part.bundled.len() as u16).to_le_bytes());
    for bf in &part.bundled {
        encode_bundled(&mut output, bf)?;
    }
    Ok(output)
}

pub fn decode_part(data: &[u8]) -> Result<PayloadPart> {
    let mut reader = Reader { data, cursor: 0 };
    let magic = reader.take(PART_MAGIC.len())?;
    if magic != PART_MAGIC {
        return Err("invalid part payload magic".into());
    }
    let format_version = reader.u16()?;
    let target_byte = reader.u8()?;
    let target = match target_byte {
        0 => TargetName::Doraemon,
        1 => TargetName::Nobita,
        2 => TargetName::Dorami,
        3 => TargetName::Shizuka,
        4 => TargetName::Suneo,
        5 => TargetName::Gian,
        6 => TargetName::Others,
        7 => TargetName::Sprites,
        8 => TargetName::Runtime,
        _ => return Err(format!("unknown target {target_byte}")),
    };
    let _target_type_byte = reader.u8()?;
    let language = match reader.u8()? {
        0 => Language::English,
        1 => Language::Vietnamese,
        2 => Language::Custom,
        _ => return Err("invalid part language".into()),
    };
    let is_empty = reader.u8()? != 0;
    let _reserved = reader.take(4)?;

    let fingerprint_count = reader.u16()? as usize;
    let mut base_fingerprints = Vec::with_capacity(fingerprint_count);
    for _ in 0..fingerprint_count {
        base_fingerprints.push(decode_required(&mut reader)?);
    }

    let file_patch_count = reader.u16()? as usize;
    let mut file_patches = Vec::with_capacity(file_patch_count);
    for _ in 0..file_patch_count {
        file_patches.push(decode_file(&mut reader)?);
    }

    let strings = if reader.u8()? == 1 {
        let id_count = reader.u32()? as usize;
        let mut expected_ids = Vec::with_capacity(id_count);
        for _ in 0..id_count {
            expected_ids.push(
                String::from_utf8(reader.bytes()?.to_vec())
                    .map_err(|_| "non-UTF-8 strings record ID".to_string())?,
            );
        }
        let replacement_count = reader.u32()? as usize;
        let mut replacements = BTreeMap::new();
        for _ in 0..replacement_count {
            let id = String::from_utf8(reader.bytes()?.to_vec())
                .map_err(|_| "non-UTF-8 strings replacement ID".to_string())?;
            let value = reader.bytes()?.to_vec();
            if !expected_ids.contains(&id) || replacements.insert(id.clone(), value).is_some() {
                return Err(format!("invalid or duplicate strings replacement {id}"));
            }
        }
        Some(StringsPatch {
            expected_ids,
            replacements,
        })
    } else {
        None
    };

    let voice = if reader.u8()? == 1 {
        let base_hash = reader.take(32)?.try_into().unwrap();
        let target_hash = reader.take(32)?.try_into().unwrap();
        let base_len = reader.u64()?;
        let target_len = reader.u64()?;
        let id_count = reader.u32()? as usize;
        let mut expected_ids = Vec::with_capacity(id_count);
        for _ in 0..id_count {
            expected_ids.push(
                String::from_utf8(reader.bytes()?.to_vec())
                    .map_err(|_| "non-UTF-8 voice record ID".to_string())?,
            );
        }
        let replacement_count = reader.u32()? as usize;
        let mut replacements = BTreeMap::new();
        for _ in 0..replacement_count {
            let id = String::from_utf8(reader.bytes()?.to_vec())
                .map_err(|_| "non-UTF-8 voice replacement ID".to_string())?;
            let packed = reader.bytes()?.to_vec();
            if packed.is_empty()
                || !expected_ids.contains(&id)
                || replacements.insert(id.clone(), packed).is_some()
            {
                return Err(format!("invalid or duplicate voice replacement {id}"));
            }
        }
        Some(VoicePatch {
            expected_ids,
            replacements,
            base_hash,
            target_hash,
            base_len,
            target_len,
        })
    } else {
        None
    };

    let executable_plain = if reader.u8()? == 1 {
        Some(decode_file(&mut reader)?)
    } else {
        None
    };

    let executable_portable = if reader.u8()? == 1 {
        Some(decode_file(&mut reader)?)
    } else {
        None
    };

    let bundled_count = reader.u16()? as usize;
    let mut bundled = Vec::with_capacity(bundled_count);
    for _ in 0..bundled_count {
        bundled.push(decode_bundled(&mut reader)?);
    }

    if reader.cursor != data.len() {
        return Err("part payload has trailing bytes".into());
    }

    Ok(PayloadPart {
        format_version,
        language,
        target,
        base_fingerprints,
        strings,
        voice,
        file_patches,
        executable_plain,
        executable_portable,
        bundled,
        is_empty,
    })
}

fn put_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    let length =
        u32::try_from(bytes.len()).map_err(|_| "payload field exceeds 4 GiB".to_string())?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn encode_file(output: &mut Vec<u8>, file: &FilePatch) -> Result<()> {
    put_bytes(output, file.name.as_bytes())?;
    output.extend_from_slice(&file.base_hash);
    output.extend_from_slice(&file.target_hash);
    output.extend_from_slice(&file.base_len.to_le_bytes());
    output.extend_from_slice(&file.target_len.to_le_bytes());
    put_bytes(output, &file.delta)
}

fn encode_required(output: &mut Vec<u8>, file: &RequiredFile) -> Result<()> {
    put_bytes(output, file.name.as_bytes())?;
    output.extend_from_slice(&file.hash);
    output.extend_from_slice(&file.len.to_le_bytes());
    Ok(())
}

fn encode_bundled(output: &mut Vec<u8>, file: &BundledFile) -> Result<()> {
    put_bytes(output, file.name.as_bytes())?;
    output.extend_from_slice(&file.hash);
    put_bytes(output, &file.bytes)
}

fn encode_profile(output: &mut Vec<u8>, profile: &PatchProfile) -> Result<()> {
    put_bytes(output, profile.name.as_bytes())?;
    output.extend_from_slice(&(profile.required.len() as u16).to_le_bytes());
    for file in &profile.required {
        encode_required(output, file)?;
    }
    output.extend_from_slice(&(profile.files.len() as u16).to_le_bytes());
    for file in &profile.files {
        encode_file(output, file)?;
    }
    output.push(profile.executable_plain.is_some() as u8);
    if let Some(file) = &profile.executable_plain {
        encode_file(output, file)?;
    }
    encode_file(output, &profile.executable_portable)
}

pub fn encode(payload: &Payload) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    output.push(match payload.language {
        Language::English => 0,
        Language::Vietnamese => 1,
        Language::Custom => 2,
    });
    output.extend_from_slice(&(payload.profiles.len() as u16).to_le_bytes());
    for profile in &payload.profiles {
        encode_profile(&mut output, profile)?;
    }
    output.push(payload.strings.is_some() as u8);
    if let Some(strings) = &payload.strings {
        output.extend_from_slice(&(strings.expected_ids.len() as u32).to_le_bytes());
        for id in &strings.expected_ids {
            put_bytes(&mut output, id.as_bytes())?;
        }
        output.extend_from_slice(&(strings.replacements.len() as u32).to_le_bytes());
        for (id, value) in &strings.replacements {
            put_bytes(&mut output, id.as_bytes())?;
            put_bytes(&mut output, value)?;
        }
    }
    output.push(payload.voice.is_some() as u8);
    if let Some(voice) = &payload.voice {
        output.extend_from_slice(&voice.base_hash);
        output.extend_from_slice(&voice.target_hash);
        output.extend_from_slice(&voice.base_len.to_le_bytes());
        output.extend_from_slice(&voice.target_len.to_le_bytes());
        output.extend_from_slice(&(voice.expected_ids.len() as u32).to_le_bytes());
        for id in &voice.expected_ids {
            put_bytes(&mut output, id.as_bytes())?;
        }
        output.extend_from_slice(&(voice.replacements.len() as u32).to_le_bytes());
        for (id, packed) in &voice.replacements {
            put_bytes(&mut output, id.as_bytes())?;
            put_bytes(&mut output, packed)?;
        }
    }
    output.extend_from_slice(&(payload.bundled.len() as u16).to_le_bytes());
    for file in &payload.bundled {
        encode_bundled(&mut output, file)?;
    }
    Ok(output)
}

struct Reader<'a> {
    data: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or_else(|| "payload offset overflow".to_string())?;
        let result = self
            .data
            .get(self.cursor..end)
            .ok_or_else(|| "truncated patch payload".to_string())?;
        self.cursor = end;
        Ok(result)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn bytes(&mut self) -> Result<&'a [u8]> {
        let len = self.u32()? as usize;
        self.take(len)
    }
}

fn decode_file(reader: &mut Reader<'_>) -> Result<FilePatch> {
    let name = String::from_utf8(reader.bytes()?.to_vec())
        .map_err(|_| "non-UTF-8 payload filename".to_string())?;
    let base_hash = reader.take(32)?.try_into().unwrap();
    let target_hash = reader.take(32)?.try_into().unwrap();
    let base_len = reader.u64()?;
    let target_len = reader.u64()?;
    let delta = reader.bytes()?.to_vec();
    Ok(FilePatch {
        name,
        base_hash,
        target_hash,
        base_len,
        target_len,
        delta,
    })
}

fn decode_required(reader: &mut Reader<'_>) -> Result<RequiredFile> {
    let name = String::from_utf8(reader.bytes()?.to_vec())
        .map_err(|_| "non-UTF-8 payload filename".to_string())?;
    let hash = reader.take(32)?.try_into().unwrap();
    let len = reader.u64()?;
    Ok(RequiredFile { name, hash, len })
}

fn decode_bundled(reader: &mut Reader<'_>) -> Result<BundledFile> {
    let name = String::from_utf8(reader.bytes()?.to_vec())
        .map_err(|_| "non-UTF-8 bundled filename".to_string())?;
    let hash = reader.take(32)?.try_into().unwrap();
    let bytes = reader.bytes()?.to_vec();
    if crate::hash::bytes(&bytes) != hash {
        return Err(format!("bundled file {name} failed verification"));
    }
    Ok(BundledFile { name, hash, bytes })
}

fn decode_profile(reader: &mut Reader<'_>) -> Result<PatchProfile> {
    let name = String::from_utf8(reader.bytes()?.to_vec())
        .map_err(|_| "non-UTF-8 profile name".to_string())?;
    let required_count = reader.u16()? as usize;
    let mut required = Vec::with_capacity(required_count);
    for _ in 0..required_count {
        required.push(decode_required(reader)?);
    }
    let count = reader.u16()? as usize;
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        files.push(decode_file(reader)?);
    }
    let executable_plain = if reader.u8()? == 1 {
        Some(decode_file(reader)?)
    } else {
        None
    };
    let executable_portable = decode_file(reader)?;
    Ok(PatchProfile {
        name,
        required,
        files,
        executable_plain,
        executable_portable,
    })
}

pub fn decode(data: &[u8]) -> Result<Payload> {
    let mut reader = Reader { data, cursor: 0 };
    let magic = reader.take(MAGIC.len())?;
    let legacy = magic == LEGACY_MAGIC;
    if magic != MAGIC && !legacy {
        return Err("invalid patch payload magic".into());
    }
    let language = match reader.u8()? {
        0 => Language::English,
        1 => Language::Vietnamese,
        2 => Language::Custom,
        _ => return Err("invalid payload language".into()),
    };
    let profile_count = reader.u16()? as usize;
    let mut profiles = Vec::with_capacity(profile_count);
    for _ in 0..profile_count {
        profiles.push(decode_profile(&mut reader)?);
    }
    let strings = if reader.u8()? == 1 {
        let id_count = reader.u32()? as usize;
        let mut expected_ids = Vec::with_capacity(id_count);
        for _ in 0..id_count {
            expected_ids.push(
                String::from_utf8(reader.bytes()?.to_vec())
                    .map_err(|_| "non-UTF-8 strings record ID".to_string())?,
            );
        }
        let replacement_count = reader.u32()? as usize;
        let mut replacements = BTreeMap::new();
        for _ in 0..replacement_count {
            let id = String::from_utf8(reader.bytes()?.to_vec())
                .map_err(|_| "non-UTF-8 strings replacement ID".to_string())?;
            let value = reader.bytes()?.to_vec();
            if value.last() != Some(&0) {
                return Err(format!("strings replacement {id} is not NUL terminated"));
            }
            if !expected_ids.contains(&id) || replacements.insert(id.clone(), value).is_some() {
                return Err(format!("invalid or duplicate strings replacement {id}"));
            }
        }
        Some(StringsPatch {
            expected_ids,
            replacements,
        })
    } else {
        None
    };
    let voice = if !legacy && reader.u8()? == 1 {
        let base_hash = reader.take(32)?.try_into().unwrap();
        let target_hash = reader.take(32)?.try_into().unwrap();
        let base_len = reader.u64()?;
        let target_len = reader.u64()?;
        let id_count = reader.u32()? as usize;
        let mut expected_ids = Vec::with_capacity(id_count);
        for _ in 0..id_count {
            expected_ids.push(
                String::from_utf8(reader.bytes()?.to_vec())
                    .map_err(|_| "non-UTF-8 voice record ID".to_string())?,
            );
        }
        let replacement_count = reader.u32()? as usize;
        let mut replacements = BTreeMap::new();
        for _ in 0..replacement_count {
            let id = String::from_utf8(reader.bytes()?.to_vec())
                .map_err(|_| "non-UTF-8 voice replacement ID".to_string())?;
            let packed = reader.bytes()?.to_vec();
            if packed.is_empty()
                || !expected_ids.contains(&id)
                || replacements.insert(id.clone(), packed).is_some()
            {
                return Err(format!("invalid or duplicate voice replacement {id}"));
            }
        }
        Some(VoicePatch {
            expected_ids,
            replacements,
            base_hash,
            target_hash,
            base_len,
            target_len,
        })
    } else {
        None
    };
    let bundled_count = reader.u16()? as usize;
    let mut bundled = Vec::with_capacity(bundled_count);
    for _ in 0..bundled_count {
        bundled.push(decode_bundled(&mut reader)?);
    }
    if reader.cursor != data.len() {
        return Err("patch payload has trailing bytes".into());
    }
    Ok(Payload {
        language,
        profiles,
        strings,
        voice,
        bundled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payload_round_trip() {
        let source = vec![1_u8; 256];
        let mut target = source.clone();
        target[80] = 9;
        let file = FilePatch::create("strings.dat", &source, &target).unwrap();
        let profile = PatchProfile {
            name: "test".into(),
            required: vec![RequiredFile {
                name: "strings.dat".into(),
                hash: crate::hash::bytes(&source),
                len: source.len() as u64,
            }],
            files: vec![file],
            executable_plain: None,
            executable_portable: FilePatch::create("Doraemon.exe", &source, &target).unwrap(),
        };
        let payload = Payload {
            language: Language::English,
            profiles: vec![profile],
            strings: Some(StringsPatch {
                expected_ids: vec!["001/041".into()],
                replacements: BTreeMap::from([("001/041".into(), b"Test\0".to_vec())]),
            }),
            voice: Some(VoicePatch {
                expected_ids: vec!["000/000/000".into()],
                replacements: BTreeMap::from([("000/000/000".into(), vec![0])]),
                base_hash: crate::hash::bytes(b"base"),
                target_hash: crate::hash::bytes(b"target"),
                base_len: 4,
                target_len: 6,
            }),
            bundled: vec![BundledFile {
                name: "ddraw.ini".into(),
                hash: crate::hash::bytes(b"hello"),
                bytes: b"hello".to_vec(),
            }],
        };
        let decoded = decode(&encode(&payload).unwrap()).unwrap();
        assert_eq!(decoded.language, Language::English);
        assert_eq!(decoded.profiles[0].files[0].apply(&source).unwrap(), target);
        assert_eq!(decoded.bundled[0].bytes, b"hello");
        assert_eq!(decoded.strings.unwrap().replacements["001/041"], b"Test\0");
        assert_eq!(decoded.voice.unwrap().replacements["000/000/000"], vec![0]);
    }

    #[test]
    fn decodes_voice_less_version_four_payloads() {
        let payload = Payload {
            language: Language::English,
            profiles: Vec::new(),
            strings: None,
            voice: None,
            bundled: Vec::new(),
        };
        let encoded = encode(&payload).unwrap();
        // Version four has no voice-presence byte between strings and bundled files.
        let mut legacy = Vec::from(&LEGACY_MAGIC[..]);
        legacy.extend_from_slice(&encoded[8..12]);
        legacy.extend_from_slice(&encoded[13..]);
        let decoded = decode(&legacy).unwrap();
        assert_eq!(decoded.language, Language::English);
        assert!(decoded.voice.is_none());
        assert!(decoded.bundled.is_empty());
    }
}

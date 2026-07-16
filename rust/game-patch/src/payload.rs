use std::collections::BTreeMap;

use crate::{delta, hash::Hash, Result};

const MAGIC: &[u8; 8] = b"DMPATCH4";

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
    pub bundled: Vec<BundledFile>,
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
    if reader.take(MAGIC.len())? != MAGIC {
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
    }
}

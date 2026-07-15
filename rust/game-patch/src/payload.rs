use crate::{delta, hash::Hash, Result};

const MAGIC: &[u8; 8] = b"DMPATCH2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    English,
    Vietnamese,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Vietnamese => "Vietnamese",
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
pub struct Payload {
    pub language: Language,
    pub required: Vec<RequiredFile>,
    pub files: Vec<FilePatch>,
    pub executable_plain: Option<FilePatch>,
    pub executable_portable: FilePatch,
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

pub fn encode(payload: &Payload) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    output.push(match payload.language {
        Language::English => 0,
        Language::Vietnamese => 1,
    });
    output.extend_from_slice(&(payload.required.len() as u16).to_le_bytes());
    for file in &payload.required {
        encode_required(&mut output, file)?;
    }
    output.extend_from_slice(&(payload.files.len() as u16).to_le_bytes());
    for file in &payload.files {
        encode_file(&mut output, file)?;
    }
    output.push(payload.executable_plain.is_some() as u8);
    if let Some(file) = &payload.executable_plain {
        encode_file(&mut output, file)?;
    }
    encode_file(&mut output, &payload.executable_portable)?;
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

pub fn decode(data: &[u8]) -> Result<Payload> {
    let mut reader = Reader { data, cursor: 0 };
    if reader.take(MAGIC.len())? != MAGIC {
        return Err("invalid patch payload magic".into());
    }
    let language = match reader.u8()? {
        0 => Language::English,
        1 => Language::Vietnamese,
        _ => return Err("invalid payload language".into()),
    };
    let required_count = reader.u16()? as usize;
    let mut required = Vec::with_capacity(required_count);
    for _ in 0..required_count {
        required.push(decode_required(&mut reader)?);
    }
    let count = reader.u16()? as usize;
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        files.push(decode_file(&mut reader)?);
    }
    let executable_plain = if reader.u8()? == 1 {
        Some(decode_file(&mut reader)?)
    } else {
        None
    };
    let executable_portable = decode_file(&mut reader)?;
    if reader.cursor != data.len() {
        return Err("patch payload has trailing bytes".into());
    }
    Ok(Payload {
        language,
        required,
        files,
        executable_plain,
        executable_portable,
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
        let payload = Payload {
            language: Language::English,
            required: vec![RequiredFile {
                name: "strings.dat".into(),
                hash: crate::hash::bytes(&source),
                len: source.len() as u64,
            }],
            files: vec![file],
            executable_plain: None,
            executable_portable: FilePatch::create("Doraemon.exe", &source, &target).unwrap(),
        };
        let decoded = decode(&encode(&payload).unwrap()).unwrap();
        assert_eq!(decoded.language, Language::English);
        assert_eq!(decoded.files[0].apply(&source).unwrap(), target);
    }
}

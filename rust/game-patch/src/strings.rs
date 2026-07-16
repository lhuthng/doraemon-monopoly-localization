use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::{payload::StringsPatch, Result};

const SIGNATURE: &[u8] = b"\0\0GameOne Systems Limited\nWritten by Samme NG\0";

#[derive(Clone)]
struct Node {
    path: Vec<u16>,
    offset: usize,
    container: bool,
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .ok_or("truncated archive integer")?
            .try_into()
            .unwrap(),
    ))
}

fn put_u32(data: &mut [u8], offset: usize, value: usize) -> Result<()> {
    let value = u32::try_from(value).map_err(|_| "archive exceeds 4 GiB".to_string())?;
    data.get_mut(offset..offset + 4)
        .ok_or("truncated archive offset table")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn signature(data: &[u8], offset: usize) -> bool {
    data.get(offset..offset + SIGNATURE.len()) == Some(SIGNATURE)
}

fn nodes(data: &[u8], offset: usize, path: &[u16], output: &mut Vec<Node>) -> Result<()> {
    if !signature(data, offset) {
        return Err(format!("missing GameOne header at {offset:#x}"));
    }
    let count = u32_at(data, offset + 0x42)? as usize;
    let table = offset + 0x66;
    if count > 100_000 || table + (count + 1) * 4 > data.len() {
        return Err(format!("invalid archive child count {count}"));
    }
    output.push(Node {
        path: path.to_vec(),
        offset,
        container: true,
    });
    for index in 0..count {
        let child = offset + u32_at(data, table + index * 4)? as usize;
        if child >= data.len() {
            return Err(format!("archive child {index} is outside the file"));
        }
        let mut child_path = path.to_vec();
        child_path.push(index as u16);
        if signature(data, child) {
            nodes(data, child, &child_path, output)?;
        } else {
            output.push(Node {
                path: child_path,
                offset: child,
                container: false,
            });
        }
    }
    Ok(())
}

fn archive_nodes(data: &[u8]) -> Result<Vec<Node>> {
    let mut output = Vec::new();
    nodes(data, 0, &[], &mut output)?;
    Ok(output)
}

struct CodeReader<'a> {
    data: &'a [u8],
    position: usize,
    bits: u64,
    bit_count: usize,
}

impl CodeReader<'_> {
    fn read(&mut self) -> Result<u16> {
        while self.bit_count < 14 {
            let byte = *self
                .data
                .get(self.position)
                .ok_or("compressed string ended before its end code")?;
            self.bits = (self.bits << 8) | byte as u64;
            self.position += 1;
            self.bit_count += 8;
        }
        self.bit_count -= 14;
        let code = ((self.bits >> self.bit_count) & 0x3fff) as u16;
        self.bits &= (1_u64 << self.bit_count).wrapping_sub(1);
        Ok(code)
    }
}

pub fn decompress(payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() < 5 {
        return Err("compressed string payload is too small".into());
    }
    let expected = u32_at(payload, 0)? as usize;
    let mut reader = CodeReader {
        data: &payload[4..],
        position: 0,
        bits: 0,
        bit_count: 0,
    };
    let mut prefix = vec![0_u16; 0x4000];
    let mut suffix = vec![0_u8; 0x4000];
    let mut next_code = 0x100_usize;
    let expand = |initial: u16, next: usize, prefix: &[u16], suffix: &[u8]| -> Result<Vec<u8>> {
        let mut code = initial as usize;
        let mut reversed = Vec::new();
        while code > 0xff {
            if code >= next {
                return Err(format!("invalid dictionary reference {code:#x}"));
            }
            reversed.push(suffix[code]);
            code = prefix[code] as usize;
            if reversed.len() >= 0xfa0 {
                return Err("compressed dictionary chain is too long".into());
            }
        }
        reversed.push(code as u8);
        reversed.reverse();
        Ok(reversed)
    };
    let mut old = reader.read()?;
    if old > 0xff {
        return Err("compressed string starts with a dictionary code".into());
    }
    let mut output = vec![old as u8];
    loop {
        let code = reader.read()?;
        if code == 0x3fff {
            break;
        }
        let expanded = if code as usize >= next_code {
            if code as usize != next_code {
                return Err(format!("future dictionary reference {code:#x}"));
            }
            let mut value = expand(old, next_code, &prefix, &suffix)?;
            value.push(value[0]);
            value
        } else {
            expand(code, next_code, &prefix, &suffix)?
        };
        output.extend_from_slice(&expanded);
        if next_code <= 0x3ffe {
            prefix[next_code] = old;
            suffix[next_code] = expanded[0];
            next_code += 1;
        }
        old = code;
    }
    if output.len() != expected {
        return Err(format!(
            "decoded {} bytes but record declares {expected}",
            output.len()
        ));
    }
    Ok(output)
}

fn pack_codes(codes: &[u16]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut bits = 0_u64;
    let mut count = 0_usize;
    for &code in codes {
        bits = (bits << 14) | code as u64;
        count += 14;
        while count >= 8 {
            count -= 8;
            output.push(((bits >> count) & 0xff) as u8);
            bits &= (1_u64 << count).wrapping_sub(1);
        }
    }
    if count > 0 {
        output.push((bits << (8 - count)) as u8);
    }
    output
}

pub fn compress(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.is_empty() {
        return Err("cannot compress an empty string".into());
    }
    let mut dictionary: HashMap<Vec<u8>, u16> =
        (0_u16..=255).map(|byte| (vec![byte as u8], byte)).collect();
    let mut next_code = 0x100_u16;
    let mut phrase = vec![bytes[0]];
    let mut codes = Vec::new();
    for &byte in &bytes[1..] {
        let mut extended = phrase.clone();
        extended.push(byte);
        if dictionary.contains_key(&extended) {
            phrase = extended;
            continue;
        }
        codes.push(dictionary[&phrase]);
        if next_code <= 0x3ffe {
            dictionary.insert(extended, next_code);
            next_code += 1;
        }
        phrase.clear();
        phrase.push(byte);
    }
    codes.push(dictionary[&phrase]);
    codes.push(0x3fff);
    let packed = pack_codes(&codes);
    let mut output = Vec::with_capacity(4 + packed.len());
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(&packed);
    if decompress(&output)? != bytes {
        return Err("internal string compression verification failed".into());
    }
    Ok(output)
}

pub fn records(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let nodes = archive_nodes(data)?;
    let mut starts: Vec<_> = nodes.iter().map(|node| node.offset).collect();
    starts.push(data.len());
    starts.sort_unstable();
    starts.dedup();
    let mut output = BTreeMap::new();
    for node in nodes.iter().filter(|node| !node.container) {
        let end = starts
            .iter()
            .copied()
            .find(|start| *start > node.offset)
            .ok_or("cannot find compressed record end")?;
        let decoded = decompress(&data[node.offset..end])?;
        if decoded.last() != Some(&0) {
            return Err(format!("record {} is not NUL terminated", id(&node.path)));
        }
        output.insert(id(&node.path), decoded);
    }
    Ok(output)
}

fn id(path: &[u16]) -> String {
    path.iter()
        .map(|part| format!("{part:03}"))
        .collect::<Vec<_>>()
        .join("/")
}

fn rebuild_container(
    original: &[u8],
    offset: usize,
    path: &[u16],
    ends: &HashMap<usize, usize>,
    replacements: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>> {
    let count = u32_at(original, offset + 0x42)? as usize;
    let table = offset + 0x66;
    let children: Vec<_> = (0..count)
        .map(|index| u32_at(original, table + index * 4).map(|value| offset + value as usize))
        .collect::<Result<_>>()?;
    let first = children.iter().copied().min().unwrap_or(table);
    let mut header = original[offset..first].to_vec();
    let mut rebuilt = Vec::new();
    for (index, child) in children.iter().copied().enumerate() {
        let mut child_path = path.to_vec();
        child_path.push(index as u16);
        if signature(original, child) {
            rebuilt.push(rebuild_container(
                original,
                child,
                &child_path,
                ends,
                replacements,
            )?);
        } else if let Some(value) = replacements.get(&id(&child_path)) {
            rebuilt.push(compress(value)?);
        } else {
            let end = *ends.get(&child).ok_or("missing original record boundary")?;
            rebuilt.push(original[child..end].to_vec());
        }
    }
    let mut cursor = header.len();
    for (index, child) in rebuilt.iter().enumerate() {
        put_u32(&mut header, 0x66 + index * 4, cursor)?;
        cursor += child.len();
    }
    put_u32(&mut header, 0x66 + count * 4, cursor)?;
    let mut output = header;
    for child in rebuilt {
        output.extend_from_slice(&child);
    }
    Ok(output)
}

fn validate(data: &[u8], offset: usize, expected_end: usize) -> Result<()> {
    if !signature(data, offset) {
        return Err("rebuilt archive header is missing".into());
    }
    let count = u32_at(data, offset + 0x42)? as usize;
    let table = offset + 0x66;
    let table_end = table + (count + 1) * 4;
    let terminal = offset + u32_at(data, table + count * 4)? as usize;
    if terminal != expected_end {
        return Err(format!(
            "rebuilt archive ends at {terminal:#x}, expected {expected_end:#x}"
        ));
    }
    let children: Vec<_> = (0..count)
        .map(|index| u32_at(data, table + index * 4).map(|value| offset + value as usize))
        .collect::<Result<_>>()?;
    for (index, child) in children.iter().copied().enumerate() {
        let end = children.get(index + 1).copied().unwrap_or(terminal);
        if child < table_end || child >= end || end > terminal {
            return Err("rebuilt archive has invalid child boundaries".into());
        }
        if signature(data, child) {
            validate(data, child, end)?;
        }
    }
    Ok(())
}

pub fn create_patch(base: &[u8], target: &[u8]) -> Result<StringsPatch> {
    let base_records = records(base)?;
    let target_records = records(target)?;
    if base_records.keys().collect::<Vec<_>>() != target_records.keys().collect::<Vec<_>>() {
        return Err("source and translated strings.dat have different record IDs".into());
    }
    let expected_ids = base_records.keys().cloned().collect();
    let replacements = target_records
        .into_iter()
        .filter(|(record_id, bytes)| base_records.get(record_id) != Some(bytes))
        .collect();
    Ok(StringsPatch {
        expected_ids,
        replacements,
    })
}

pub fn apply_patch(source: &[u8], patch: &StringsPatch) -> Result<Vec<u8>> {
    let source_records = records(source)?;
    let ids: Vec<_> = source_records.keys().cloned().collect();
    if ids != patch.expected_ids {
        return Err("strings.dat record layout does not match this game release".into());
    }
    for id in patch.replacements.keys() {
        if !source_records.contains_key(id) {
            return Err(format!("strings.dat has no record {id}"));
        }
    }
    let nodes = archive_nodes(source)?;
    let mut starts: Vec<_> = nodes.iter().map(|node| node.offset).collect();
    starts.push(source.len());
    starts.sort_unstable();
    starts.dedup();
    let ends = starts
        .windows(2)
        .map(|pair| (pair[0], pair[1]))
        .collect::<HashMap<_, _>>();
    let rebuilt = rebuild_container(source, 0, &[], &ends, &patch.replacements)?;
    validate(&rebuilt, 0, rebuilt.len())?;
    let verified = records(&rebuilt)?;
    for (record_id, expected) in &patch.replacements {
        if verified.get(record_id) != Some(expected) {
            return Err(format!("record {record_id} failed rebuilt verification"));
        }
    }
    Ok(rebuilt)
}

pub fn matches(source: &[u8], patch: &StringsPatch) -> Result<bool> {
    let parsed = records(source)?;
    let ids = parsed.keys().cloned().collect::<BTreeSet<_>>();
    if ids != patch.expected_ids.iter().cloned().collect() {
        return Ok(false);
    }
    Ok(patch
        .replacements
        .iter()
        .all(|(record_id, expected)| parsed.get(record_id) == Some(expected)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_patch_handles_two_archives_with_the_same_record_layout() {
        let (Ok(base_path), Ok(alternate_path), Ok(target_path)) = (
            std::env::var("DORAEMON_TEST_STRINGS_BASE"),
            std::env::var("DORAEMON_TEST_STRINGS_ALTERNATE"),
            std::env::var("DORAEMON_TEST_STRINGS_TARGET"),
        ) else {
            return;
        };
        let base = std::fs::read(base_path).unwrap();
        let alternate = std::fs::read(alternate_path).unwrap();
        let target = std::fs::read(target_path).unwrap();
        let patch = create_patch(&base, &target).unwrap();
        let alternate_before = records(&alternate).unwrap();
        let rebuilt = apply_patch(&alternate, &patch).unwrap();
        let alternate_after = records(&rebuilt).unwrap();
        let target_records = records(&target).unwrap();
        for id in &patch.expected_ids {
            if patch.replacements.contains_key(id) {
                assert_eq!(alternate_after[id], target_records[id], "record {id}");
            } else {
                assert_eq!(alternate_after[id], alternate_before[id], "record {id}");
            }
        }
        assert!(matches(&rebuilt, &patch).unwrap());
    }
}

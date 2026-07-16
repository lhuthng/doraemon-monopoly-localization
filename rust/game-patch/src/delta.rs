use std::collections::HashMap;

use crate::Result;

const MAGIC: &[u8; 4] = b"DMD1";
const BLOCK: usize = 32;
const MIN_COPY: usize = 32;

fn hash_block(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn put_u32(output: &mut Vec<u8>, value: usize) -> Result<()> {
    let value = u32::try_from(value).map_err(|_| "delta field exceeds 4 GiB".to_string())?;
    output.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn flush_literal(output: &mut Vec<u8>, literal: &mut Vec<u8>) -> Result<()> {
    if literal.is_empty() {
        return Ok(());
    }
    output.push(1);
    put_u32(output, literal.len())?;
    output.append(literal);
    Ok(())
}

pub fn build(source: &[u8], target: &[u8]) -> Result<Vec<u8>> {
    let mut blocks: HashMap<u64, Vec<usize>> = HashMap::new();
    if source.len() >= BLOCK {
        for offset in (0..=source.len() - BLOCK).step_by(BLOCK) {
            blocks
                .entry(hash_block(&source[offset..offset + BLOCK]))
                .or_default()
                .push(offset);
        }
    }

    let mut output = Vec::with_capacity(target.len() / 4);
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&(target.len() as u64).to_le_bytes());
    let mut cursor = 0;
    let mut literal = Vec::new();
    while cursor < target.len() {
        let mut best = (0_usize, 0_usize);
        if cursor + BLOCK <= target.len() {
            if let Some(candidates) = blocks.get(&hash_block(&target[cursor..cursor + BLOCK])) {
                for &source_offset in candidates.iter().take(64) {
                    if source[source_offset..source_offset + BLOCK]
                        != target[cursor..cursor + BLOCK]
                    {
                        continue;
                    }
                    let mut length = BLOCK;
                    while source_offset + length < source.len()
                        && cursor + length < target.len()
                        && source[source_offset + length] == target[cursor + length]
                    {
                        length += 1;
                    }
                    if length > best.1 {
                        best = (source_offset, length);
                    }
                }
            }
        }
        if best.1 >= MIN_COPY {
            flush_literal(&mut output, &mut literal)?;
            output.push(0);
            put_u32(&mut output, best.0)?;
            put_u32(&mut output, best.1)?;
            cursor += best.1;
        } else {
            literal.push(target[cursor]);
            cursor += 1;
        }
    }
    flush_literal(&mut output, &mut literal)?;
    output.push(0xff);
    Ok(output)
}

fn take<'a>(data: &'a [u8], cursor: &mut usize, count: usize) -> Result<&'a [u8]> {
    let end = cursor
        .checked_add(count)
        .ok_or_else(|| "delta offset overflow".to_string())?;
    let value = data
        .get(*cursor..end)
        .ok_or_else(|| "truncated delta".to_string())?;
    *cursor = end;
    Ok(value)
}

fn read_u32(data: &[u8], cursor: &mut usize) -> Result<usize> {
    Ok(u32::from_le_bytes(take(data, cursor, 4)?.try_into().unwrap()) as usize)
}

pub fn apply(source: &[u8], delta: &[u8]) -> Result<Vec<u8>> {
    if delta.get(..4) != Some(MAGIC) {
        return Err("invalid delta magic".into());
    }
    let target_len = u64::from_le_bytes(
        delta
            .get(4..12)
            .ok_or("truncated delta header")?
            .try_into()
            .unwrap(),
    );
    let target_len = usize::try_from(target_len).map_err(|_| "target is too large".to_string())?;
    let mut cursor = 12;
    let mut output = Vec::with_capacity(target_len);
    loop {
        let opcode = *take(delta, &mut cursor, 1)?.first().unwrap();
        match opcode {
            0 => {
                let offset = read_u32(delta, &mut cursor)?;
                let length = read_u32(delta, &mut cursor)?;
                output.extend_from_slice(
                    source
                        .get(offset..offset + length)
                        .ok_or("delta copy outside source")?,
                );
            }
            1 => {
                let length = read_u32(delta, &mut cursor)?;
                output.extend_from_slice(take(delta, &mut cursor, length)?);
            }
            0xff => break,
            other => return Err(format!("unknown delta opcode {other:#x}")),
        }
    }
    if output.len() != target_len {
        return Err(format!(
            "delta produced {} bytes; expected {target_len}",
            output.len()
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_insertions_and_changes() {
        let source = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".repeat(20);
        let mut target = source.clone();
        target.splice(80..93, b"Doraemon Monopoly".iter().copied());
        target.extend_from_slice(b"tail");
        let patch = build(&source, &target).unwrap();
        assert_eq!(apply(&source, &patch).unwrap(), target);
        assert!(patch.len() < target.len());
    }
}

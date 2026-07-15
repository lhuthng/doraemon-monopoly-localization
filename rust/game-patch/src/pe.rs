use std::collections::HashMap;

use crate::{hash, Result};

pub const EXPECTED_EXE_SHA256: &str =
    "fdf00e681671f93b09d257f77d7ce0720e7129cf6bc44ba9e0f19c2efa4fecba";
const IMAGE_BASE: u32 = 0x0040_0000;
const CSEG_VA: u32 = 0x004d_1000;
const CSEG_RAW: usize = 0x000c_c000;
const CAVE_VA: u32 = 0x004d_2c00;
const CAVE_RAW: usize = 0x000c_dc00;
const PORT_VA: u32 = 0x004d_6000;
const PORT_RAW: usize = 0x000d_1000;
const PORT_SIZE: usize = 0x1000;

#[derive(Clone)]
enum Target {
    Label(&'static str),
    Address(u32),
}
#[derive(Clone)]
struct Fixup {
    offset: usize,
    target: Target,
    relative: bool,
}

#[derive(Default)]
struct Asm {
    base: u32,
    bytes: Vec<u8>,
    labels: HashMap<&'static str, u32>,
    fixups: Vec<Fixup>,
}

impl Asm {
    fn new(base: u32) -> Self {
        Self {
            base,
            ..Self::default()
        }
    }
    fn va(&self) -> u32 {
        self.base + self.bytes.len() as u32
    }
    fn emit(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    fn u32(&mut self, value: u32) {
        self.emit(&value.to_le_bytes());
    }
    fn label(&mut self, name: &'static str) {
        self.labels.insert(name, self.va());
    }
    fn fix(&mut self, opcode: &[u8], target: Target, relative: bool) {
        self.emit(opcode);
        self.fixups.push(Fixup {
            offset: self.bytes.len(),
            target,
            relative,
        });
        self.u32(0);
    }
    fn jmp(&mut self, target: Target) {
        self.fix(&[0xe9], target, true);
    }
    fn je(&mut self, target: Target) {
        self.fix(&[0x0f, 0x84], target, true);
    }
    fn jne(&mut self, target: Target) {
        self.fix(&[0x0f, 0x85], target, true);
    }
    fn jb(&mut self, target: Target) {
        self.fix(&[0x0f, 0x82], target, true);
    }
    fn ja(&mut self, target: Target) {
        self.fix(&[0x0f, 0x87], target, true);
    }
    fn jbe(&mut self, target: Target) {
        self.fix(&[0x0f, 0x86], target, true);
    }
    fn absolute(&mut self, opcode: &[u8], label: &'static str) {
        self.fix(opcode, Target::Label(label), false);
    }
    fn call_iat(&mut self, address: u32) {
        self.emit(&[0xff, 0x15]);
        self.u32(address);
    }
    fn finish(mut self) -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
        for fixup in &self.fixups {
            let target = match fixup.target {
                Target::Label(name) => *self
                    .labels
                    .get(name)
                    .ok_or_else(|| format!("missing assembler label {name}"))?,
                Target::Address(address) => address,
            };
            let value = if fixup.relative {
                target.wrapping_sub(self.base + fixup.offset as u32 + 4)
            } else {
                target
            };
            self.bytes[fixup.offset..fixup.offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        Ok((self.bytes, self.labels))
    }
}

fn expect_supported(input: &[u8]) -> Result<()> {
    let expected = hash::parse(EXPECTED_EXE_SHA256)?;
    if hash::bytes(input) != expected {
        return Err(format!(
            "unsupported Doraemon.exe (SHA-256 {})",
            hash::hex(&hash::bytes(input))
        ));
    }
    Ok(())
}

fn patch_jump(output: &mut [u8], va: u32, target: u32, replaced: usize) -> Result<()> {
    let raw = (va - IMAGE_BASE) as usize;
    let bytes = output
        .get_mut(raw..raw + replaced)
        .ok_or("executable jump patch is outside file")?;
    bytes.fill(0x90);
    bytes[0] = 0xe9;
    bytes[1..5].copy_from_slice(&target.wrapping_sub(va + 5).to_le_bytes());
    Ok(())
}

fn patch_cseg_jump(output: &mut [u8], va: u32, target: u32) -> Result<()> {
    let raw = CSEG_RAW + (va - CSEG_VA) as usize;
    let bytes = output
        .get_mut(raw..raw + 5)
        .ok_or("CSEG jump patch is outside file")?;
    bytes[0] = 0xe9;
    bytes[1..5].copy_from_slice(&target.wrapping_sub(va + 5).to_le_bytes());
    Ok(())
}

fn prefix_check(a: &mut Asm, chinese: &'static str, check_cd: &'static str, second: &'static str) {
    a.emit(&[0x3c, 0xcc]);
    a.jne(Target::Label(check_cd));
    a.jmp(Target::Label(second));
    a.label(check_cd);
    a.emit(&[0x3c, 0xcd]);
    a.jne(Target::Label(chinese));
    a.label(second);
    a.emit(&[0xf6, 0x06, 0x80]);
    a.jne(Target::Label(chinese));
}

fn vietnamese_index(a: &mut Asm) {
    a.emit(&[
        0x0f, 0xb6, 0xc0, 0x2d, 0xcc, 0, 0, 0, 0xc1, 0xe0, 0x07, 0x0f, 0xb6, 0x0e, 0x01, 0xc8,
    ]);
    a.emit(&[0x8b, 0x0d]);
    a.u32(0x004d_001a);
    a.emit(&[
        0x83, 0xf9, 0x04, 0x76, 0x02, 0x31, 0xc9, 0xc1, 0xe1, 0x08, 0x01, 0xc8, 0x05,
    ]);
    a.u32(640);
}

fn vietnamese_cave() -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
    let mut a = Asm::new(CAVE_VA);
    a.label("measure_string");
    prefix_check(&mut a, "measure_chinese", "m_cd", "m_second");
    vietnamese_index(&mut a);
    a.emit(&[0x8b, 0x0d]);
    a.u32(0x004d_0006);
    a.emit(&[
        0x8b, 0x44, 0x81, 0x02, 0x01, 0xc8, 0x0f, 0xb6, 0, 0x01, 0xc2, 0x46,
    ]);
    a.jmp(Target::Address(0x004d_11a8));
    a.label("measure_chinese");
    a.emit(&[0x46, 0x83, 0xc2, 0x10]);
    a.jmp(Target::Address(0x004d_11a8));

    a.label("character_width");
    a.emit(&[0x51]);
    prefix_check(&mut a, "width_chinese", "w_cd", "w_second");
    vietnamese_index(&mut a);
    a.emit(&[0x8b, 0x0d]);
    a.u32(0x004d_0006);
    a.emit(&[0x8b, 0x44, 0x81, 0x02, 0x01, 0xc8, 0x0f, 0xb6, 0, 0x59]);
    a.jmp(Target::Address(0x004d_123a));
    a.label("width_chinese");
    a.emit(&[0xb8, 0x10, 0, 0, 0, 0x59]);
    a.jmp(Target::Address(0x004d_123a));

    a.label("single_render");
    prefix_check(&mut a, "single_chinese", "s_cd", "s_second");
    vietnamese_index(&mut a);
    a.emit(&[0x46, 0x8b, 0x0d]);
    a.u32(0x004d_0006);
    a.emit(&[0x8b, 0x44, 0x81, 0x02, 0x01, 0xc8, 0x89, 0xc6]);
    a.jmp(Target::Address(0x004d_1281));
    a.label("single_chinese");
    a.emit(&[0x88, 0xc4, 0xac, 0x66, 0x25, 0xff, 0x7f]);
    a.jmp(Target::Address(0x004d_12e8));

    a.label("string_render");
    prefix_check(&mut a, "string_chinese", "r_cd", "r_second");
    vietnamese_index(&mut a);
    a.emit(&[0x46, 0x56, 0x8b, 0x0d]);
    a.u32(0x004d_0006);
    a.emit(&[0x8b, 0x44, 0x81, 0x02, 0x01, 0xc8, 0x89, 0xc6]);
    a.jmp(Target::Address(0x004d_140b));
    a.label("string_chinese");
    a.emit(&[0x88, 0xc4, 0xac, 0x56, 0x66, 0x25, 0xff, 0x7f]);
    a.jmp(Target::Address(0x004d_144c));
    a.finish()
}

fn find_section(output: &[u8], wanted: &[u8]) -> Result<usize> {
    let pe = u32::from_le_bytes(
        output
            .get(0x3c..0x40)
            .ok_or("truncated DOS header")?
            .try_into()
            .unwrap(),
    ) as usize;
    let count = u16::from_le_bytes(
        output
            .get(pe + 6..pe + 8)
            .ok_or("truncated PE header")?
            .try_into()
            .unwrap(),
    ) as usize;
    let optional = u16::from_le_bytes(
        output
            .get(pe + 20..pe + 22)
            .ok_or("truncated PE header")?
            .try_into()
            .unwrap(),
    ) as usize;
    let table = pe + 24 + optional;
    for index in 0..count {
        let offset = table + index * 40;
        let name = output
            .get(offset..offset + 8)
            .ok_or("truncated section table")?;
        if name.starts_with(wanted) {
            return Ok(offset);
        }
    }
    Err(format!(
        "PE has no {} section",
        String::from_utf8_lossy(wanted)
    ))
}

pub fn patch_vietnamese(original: &[u8]) -> Result<Vec<u8>> {
    expect_supported(original)?;
    let mut output = original.to_vec();
    let (cave, labels) = vietnamese_cave()?;
    if cave.len() > 0x400 {
        return Err(format!(
            "Vietnamese patch uses {}/1024 cave bytes",
            cave.len()
        ));
    }
    output[CAVE_RAW..CAVE_RAW + 0x400].fill(0x90);
    output[CAVE_RAW..CAVE_RAW + cave.len()].copy_from_slice(&cave);
    for (source, label) in [
        (0x004d_11d0, "measure_string"),
        (0x004d_1235, "character_width"),
        (0x004d_12e1, "single_render"),
        (0x004d_1444, "string_render"),
    ] {
        patch_cseg_jump(&mut output, source, labels[label])?;
    }
    let section = find_section(&output, b"CSEG")?;
    output[section + 8..section + 12].copy_from_slice(&0x2000_u32.to_le_bytes());
    let flags =
        u32::from_le_bytes(output[section + 36..section + 40].try_into().unwrap()) | 0x2000_0000;
    output[section + 36..section + 40].copy_from_slice(&flags.to_le_bytes());
    // Deliberately retain the original `sysfont.dat` filename.
    Ok(output)
}

const TRACK_TIMES: [(u32, u32); 10] = [
    (0, 341307),
    (341307, 471280),
    (471280, 513227),
    (513227, 621840),
    (621840, 629733),
    (629733, 726347),
    (726347, 796267),
    (796267, 858707),
    (858707, 945053),
    (945053, 959067),
];

fn portable_section() -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
    let mut a = Asm::new(PORT_VA);
    const IAT_MODULE: u32 = 0x004b_90d0;
    const IAT_MCI: u32 = 0x004b_929c;
    const CD_ROOT: u32 = 0x004c_cdf8;
    a.label("no_disc");
    a.emit(&[0x56, 0x57, 0x68, 0, 1, 0, 0, 0xa1]);
    a.u32(CD_ROOT);
    a.emit(&[0x50, 0x6a, 0]);
    a.call_iat(IAT_MODULE);
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("no_disc_failed"));
    a.emit(&[0x8b, 0x3d]);
    a.u32(CD_ROOT);
    a.emit(&[0x01, 0xc7]);
    a.label("no_disc_scan");
    a.emit(&[0x3b, 0x3d]);
    a.u32(CD_ROOT);
    a.jbe(Target::Label("no_disc_failed"));
    a.emit(&[0x4f, 0x80, 0x3f, 0x5c]);
    a.je(Target::Label("no_disc_found"));
    a.emit(&[0x80, 0x3f, 0x2f]);
    a.jne(Target::Label("no_disc_scan"));
    a.label("no_disc_found");
    a.emit(&[0x47, 0xc6, 0x07, 0]);
    a.jmp(Target::Label("no_disc_done"));
    a.label("no_disc_failed");
    a.emit(&[0xa1]);
    a.u32(CD_ROOT);
    a.emit(&[0xc7, 0, 0x2e, 0x5c, 0, 0]);
    a.label("no_disc_done");
    a.emit(&[0x5f, 0x5e]);
    a.jmp(Target::Address(0x0043_72e8));

    a.label("open_wav");
    a.emit(&[0x56, 0x57, 0x68, 0, 1, 0, 0]);
    a.absolute(&[0x68], "path_buffer");
    a.emit(&[0x6a, 0]);
    a.call_iat(IAT_MODULE);
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("open_fallback"));
    a.absolute(&[0xbf], "path_buffer");
    a.emit(&[0x01, 0xc7]);
    a.label("path_scan");
    a.absolute(&[0x81, 0xff], "path_buffer");
    a.jbe(Target::Label("open_fallback"));
    a.emit(&[0x4f, 0x80, 0x3f, 0x5c]);
    a.je(Target::Label("path_found"));
    a.emit(&[0x80, 0x3f, 0x2f]);
    a.jne(Target::Label("path_scan"));
    a.label("path_found");
    a.emit(&[0x47]);
    a.jmp(Target::Label("copy_name"));
    a.label("open_fallback");
    a.absolute(&[0xbf], "path_buffer");
    a.label("copy_name");
    a.absolute(&[0xbe], "music_name");
    a.label("copy_loop");
    a.emit(&[0xac, 0xaa, 0x84, 0xc0]);
    a.jne(Target::Label("copy_loop"));
    a.emit(&[0xc7, 0x45, 0xdc, 0x0a, 0x02, 0, 0]);
    a.absolute(&[0xc7, 0x45, 0xe0], "path_buffer");
    a.emit(&[
        0x8d, 0x45, 0xd4, 0x50, 0x68, 0, 0x32, 0, 0, 0x68, 0x03, 0x08, 0, 0, 0x6a, 0,
    ]);
    a.call_iat(IAT_MCI);
    a.emit(&[
        0x89, 0x45, 0xe8, 0x8b, 0x55, 0x90, 0x66, 0x8b, 0x45, 0xd8, 0x66, 0x89, 0x42, 0x02,
    ]);
    a.emit(&[
        0x31, 0xc0, 0x89, 0x45, 0xec, 0x89, 0x45, 0xf0, 0x89, 0x45, 0xf4, 0x8d, 0x55, 0xec, 0x52,
        0x68, 0, 0x04, 0, 0, 0x68, 0x0d, 0x08, 0, 0, 0x8b, 0x45, 0x90, 0x0f, 0xb7, 0x48, 0x02,
        0x51,
    ]);
    a.call_iat(IAT_MCI);
    a.emit(&[0x5f, 0x5e]);
    a.jmp(Target::Address(0x0048_50af));

    a.label("play");
    a.emit(&[
        0x55, 0x8b, 0xec, 0x83, 0xec, 0x18, 0x89, 0x4d, 0xfc, 0x83, 0x79, 0x10, 0,
    ]);
    a.je(Target::Label("play_reject"));
    a.emit(&[0x8b, 0x45, 0x08, 0x25, 0xff, 0, 0, 0, 0x83, 0xf8, 0x02]);
    a.jb(Target::Label("play_reject"));
    a.emit(&[0x83, 0xf8, 0x0b]);
    a.ja(Target::Label("play_reject"));
    a.emit(&[0x83, 0xe8, 0x02, 0xc1, 0xe0, 0x03]);
    a.absolute(&[0x05], "track_times");
    a.emit(&[
        0x8b, 0x10, 0x89, 0x55, 0xf4, 0x8b, 0x50, 0x04, 0x89, 0x55, 0xf8, 0x31, 0xd2, 0x89, 0x55,
        0xf0, 0x8d, 0x45, 0xf0, 0x50, 0x6a, 0x0c, 0x68, 0x06, 0x08, 0, 0, 0x8b, 0x4d, 0xfc, 0x0f,
        0xb7, 0x51, 0x02, 0x52,
    ]);
    a.call_iat(IAT_MCI);
    a.emit(&[0x89, 0x45, 0xec, 0x85, 0xc0]);
    a.jne(Target::Label("play_return"));
    a.emit(&[
        0x8b, 0x4d, 0xfc, 0x8a, 0x55, 0x08, 0x88, 0x11, 0x83, 0x7d, 0x0c, 0,
    ]);
    a.je(Target::Label("play_return"));
    a.emit(&[
        0x8b, 0x45, 0xf8, 0x2b, 0x45, 0xf4, 0x31, 0xd2, 0xb9, 0xe8, 0x03, 0, 0, 0xf7, 0xf1, 0x8b,
        0x4d, 0xfc, 0x89, 0x41, 0x14, 0xc7, 0x41, 0x0c, 1, 0, 0, 0,
    ]);
    a.label("play_return");
    a.emit(&[0x8b, 0x45, 0xec, 0x8b, 0xe5, 0x5d, 0xc2, 0x08, 0]);
    a.label("play_reject");
    a.emit(&[0xb8, 1, 0, 0, 0, 0x8b, 0xe5, 0x5d, 0xc2, 0x08, 0]);

    a.label("duration");
    a.emit(&[
        0x55, 0x8b, 0xec, 0x8b, 0x45, 0x08, 0x25, 0xff, 0, 0, 0, 0x83, 0xf8, 0x02,
    ]);
    a.jb(Target::Label("duration_zero"));
    a.emit(&[0x83, 0xf8, 0x0b]);
    a.ja(Target::Label("duration_zero"));
    a.emit(&[0x83, 0xe8, 0x02, 0xc1, 0xe0, 0x03]);
    a.absolute(&[0x05], "track_times");
    a.emit(&[
        0x8b, 0x50, 0x04, 0x2b, 0x10, 0x89, 0xd0, 0x31, 0xd2, 0xb9, 0xe8, 0x03, 0, 0, 0xf7, 0xf1,
        0x5d, 0xc2, 0x04, 0,
    ]);
    a.label("duration_zero");
    a.emit(&[0x31, 0xc0, 0x5d, 0xc2, 0x04, 0]);
    a.label("track_count");
    a.emit(&[0xb8, 0x0b, 0, 0, 0, 0xc3]);
    while a.bytes.len() % 4 != 0 {
        a.emit(&[0x90]);
    }
    a.label("track_times");
    for (start, end) in TRACK_TIMES {
        a.u32(start);
        a.u32(end);
    }
    a.label("music_name");
    a.emit(b"DoraemonMusic.wav\0");
    a.label("path_buffer");
    a.emit(&[0_u8; 256]);
    a.finish()
}

fn expect_bytes(output: &[u8], raw: usize, expected: &[u8]) -> Result<()> {
    if output.get(raw..raw + expected.len()) != Some(expected) {
        return Err(format!("unexpected executable bytes at {raw:#x}"));
    }
    Ok(())
}

fn add_section(input: &[u8], section: &[u8]) -> Result<Vec<u8>> {
    if input.len() != PORT_RAW {
        return Err(format!("unexpected executable size {}", input.len()));
    }
    if section.len() > PORT_SIZE {
        return Err(format!(
            "portable section uses {}/{} bytes",
            section.len(),
            PORT_SIZE
        ));
    }
    let mut output = vec![0x90_u8; PORT_RAW + PORT_SIZE];
    output[..input.len()].copy_from_slice(input);
    output[PORT_RAW..PORT_RAW + section.len()].copy_from_slice(section);
    let pe = u32::from_le_bytes(output[0x3c..0x40].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(output[pe + 6..pe + 8].try_into().unwrap()) as usize;
    let optional = u16::from_le_bytes(output[pe + 20..pe + 22].try_into().unwrap()) as usize;
    let table = pe + 24 + optional;
    let header = table + count * 40;
    let headers_size =
        u32::from_le_bytes(output[pe + 24 + 60..pe + 24 + 64].try_into().unwrap()) as usize;
    if header + 40 > headers_size {
        return Err("no room for portable PE section header".into());
    }
    output[header..header + 8].copy_from_slice(b".port\0\0\0");
    for (offset, value) in [
        (8, section.len() as u32),
        (12, PORT_VA - IMAGE_BASE),
        (16, PORT_SIZE as u32),
        (20, PORT_RAW as u32),
        (36, 0xe000_0060),
    ] {
        output[header + offset..header + offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    output[pe + 6..pe + 8].copy_from_slice(&((count + 1) as u16).to_le_bytes());
    output[pe + 24 + 56..pe + 24 + 60].copy_from_slice(&0x000d_7000_u32.to_le_bytes());
    output[pe + 24 + 64..pe + 24 + 68].fill(0);
    Ok(output)
}

pub fn patch_portable(verified: &[u8]) -> Result<Vec<u8>> {
    expect_bytes(verified, 0x2cc11, &[0x33, 0xc0, 0xe9, 0x35, 0x02, 0, 0])?;
    expect_bytes(verified, 0x3723a, &[0xff, 0x15, 0xac, 0x90, 0x4b, 0])?;
    for (raw, bytes) in [
        (0x85043, &[0x8b, 0x55, 0x08, 0x89, 0x55][..]),
        (0x85288, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
        (0x8545f, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
        (0x855f3, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
    ] {
        expect_bytes(verified, raw, bytes)?;
    }
    let (section, labels) = portable_section()?;
    let mut output = add_section(verified, &section)?;
    output[0x2cc11..0x2cc18].copy_from_slice(&[0xc7, 0x45, 0xf4, 0, 0, 0, 0]);
    for (source, label, replaced) in [
        (0x0043_723a, "no_disc", 6),
        (0x0048_5043, "open_wav", 5),
        (0x0048_5288, "play", 5),
        (0x0048_545f, "duration", 5),
        (0x0048_55f3, "track_count", 5),
    ] {
        patch_jump(&mut output, source, labels[label], replaced)?;
    }
    Ok(output)
}

pub fn build_variants(original: &[u8], vietnamese: bool) -> Result<(Option<Vec<u8>>, Vec<u8>)> {
    expect_supported(original)?;
    if vietnamese {
        let plain = patch_vietnamese(original)?;
        let portable = patch_portable(&plain)?;
        Ok((Some(plain), portable))
    } else {
        Ok((None, patch_portable(original)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_executable() {
        assert!(build_variants(&[0; 128], false)
            .unwrap_err()
            .contains("unsupported"));
    }
    #[test]
    fn real_fixture_matches_verified_portable_patch_when_available() {
        let Ok(folder) = std::env::var("DORAEMON_TEST_DATA_DIR") else {
            return;
        };
        let original = std::fs::read(std::path::Path::new(&folder).join("Doraemon.exe")).unwrap();
        let (_, portable) = build_variants(&original, false).unwrap();
        assert_eq!(
            hash::hex(&hash::bytes(&portable)),
            "8f8cbda0f70732c21db7385d6366cb24c94801e94dfd29f1202184796a54d7e5"
        );
        let (plain_vi, portable_vi) = build_variants(&original, true).unwrap();
        assert_eq!(plain_vi.as_ref().unwrap().len(), original.len());
        assert_eq!(portable_vi.len(), original.len() + PORT_SIZE);
        assert_eq!(&plain_vi.unwrap()[0xcb00a..0xcb016], b"sysfont.dat\0");
    }
}

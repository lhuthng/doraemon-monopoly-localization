use std::collections::HashMap;

use crate::Result;
const IMAGE_BASE: u32 = 0x0040_0000;
const CSEG_VA: u32 = 0x004d_1000;
const CSEG_RAW: usize = 0x000c_c000;
const PORT_VA: u32 = 0x004d_6000;
const PORT_SIZE: usize = 0x1000;
const PORT_TITLE_OFFSET: usize = 0x0f00;
const TITLE_VERSION: &[u8] = b"Version 1.26\0";
const TITLE_CREDIT: &[u8] = b"Version 1.26 - Patched by Thang\0";

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

fn patch_cseg_jump(
    output: &mut [u8],
    cseg_va: u32,
    cseg_raw: usize,
    va: u32,
    target: u32,
) -> Result<()> {
    let raw = cseg_raw + (va - cseg_va) as usize;
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

fn vietnamese_index(a: &mut Asm, dseg_va: u32) {
    a.emit(&[
        0x0f, 0xb6, 0xc0, 0x2d, 0xcc, 0, 0, 0, 0xc1, 0xe0, 0x07, 0x0f, 0xb6, 0x0e, 0x01, 0xc8,
    ]);
    a.emit(&[0x8b, 0x0d]);
    a.u32(dseg_va + 0x1a);
    a.emit(&[
        0x83, 0xf9, 0x04, 0x76, 0x02, 0x31, 0xc9, 0xc1, 0xe1, 0x08, 0x01, 0xc8, 0x05,
    ]);
    a.u32(640);
}

fn vietnamese_cave(
    cave_va: u32,
    dseg_va: u32,
    cseg_va: u32,
) -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
    let mut a = Asm::new(cave_va);
    a.label("measure_string");
    prefix_check(&mut a, "measure_chinese", "m_cd", "m_second");
    vietnamese_index(&mut a, dseg_va);
    a.emit(&[0x8b, 0x0d]);
    a.u32(dseg_va + 0x06);
    a.emit(&[
        0x8b, 0x44, 0x81, 0x02, 0x01, 0xc8, 0x0f, 0xb6, 0, 0x01, 0xc2, 0x46,
    ]);
    a.jmp(Target::Address(cseg_va + 0x01a8));
    a.label("measure_chinese");
    a.emit(&[0x46, 0x83, 0xc2, 0x10]);
    a.jmp(Target::Address(cseg_va + 0x01a8));

    a.label("character_width");
    a.emit(&[0x51]);
    prefix_check(&mut a, "width_chinese", "w_cd", "w_second");
    vietnamese_index(&mut a, dseg_va);
    a.emit(&[0x8b, 0x0d]);
    a.u32(dseg_va + 0x06);
    a.emit(&[0x8b, 0x44, 0x81, 0x02, 0x01, 0xc8, 0x0f, 0xb6, 0, 0x59]);
    a.jmp(Target::Address(cseg_va + 0x023a));
    a.label("width_chinese");
    a.emit(&[0xb8, 0x10, 0, 0, 0, 0x59]);
    a.jmp(Target::Address(cseg_va + 0x023a));

    a.label("single_render");
    prefix_check(&mut a, "single_chinese", "s_cd", "s_second");
    vietnamese_index(&mut a, dseg_va);
    a.emit(&[0x46, 0x8b, 0x0d]);
    a.u32(dseg_va + 0x06);
    a.emit(&[0x8b, 0x44, 0x81, 0x02, 0x01, 0xc8, 0x89, 0xc6]);
    a.jmp(Target::Address(cseg_va + 0x0281));
    a.label("single_chinese");
    a.emit(&[0x88, 0xc4, 0xac, 0x66, 0x25, 0xff, 0x7f]);
    a.jmp(Target::Address(cseg_va + 0x02e8));

    a.label("string_render");
    prefix_check(&mut a, "string_chinese", "r_cd", "r_second");
    vietnamese_index(&mut a, dseg_va);
    a.emit(&[0x46, 0x56, 0x8b, 0x0d]);
    a.u32(dseg_va + 0x06);
    a.emit(&[0x8b, 0x44, 0x81, 0x02, 0x01, 0xc8, 0x89, 0xc6]);
    a.jmp(Target::Address(cseg_va + 0x040b));
    a.label("string_chinese");
    a.emit(&[0x88, 0xc4, 0xac, 0x56, 0x66, 0x25, 0xff, 0x7f]);
    a.jmp(Target::Address(cseg_va + 0x044c));
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

#[derive(Clone, Copy)]
struct FontLayout {
    dseg_va: u32,
    cseg_va: u32,
    cseg_raw: usize,
}

fn section_location(input: &[u8], name: &[u8]) -> Result<(u32, usize, usize)> {
    let header = find_section(input, name)?;
    let pe = u32::from_le_bytes(
        input
            .get(0x3c..0x40)
            .ok_or("truncated DOS header")?
            .try_into()
            .unwrap(),
    ) as usize;
    let image_base = u32::from_le_bytes(
        input
            .get(pe + 24 + 28..pe + 24 + 32)
            .ok_or("truncated PE optional header")?
            .try_into()
            .unwrap(),
    );
    let relative_va = u32::from_le_bytes(input[header + 12..header + 16].try_into().unwrap());
    let raw_size = u32::from_le_bytes(input[header + 16..header + 20].try_into().unwrap()) as usize;
    let raw = u32::from_le_bytes(input[header + 20..header + 24].try_into().unwrap()) as usize;
    if raw
        .checked_add(raw_size)
        .map_or(true, |end| end > input.len())
    {
        return Err(format!(
            "{} section extends outside the executable",
            String::from_utf8_lossy(name)
        ));
    }
    Ok((image_base + relative_va, raw, raw_size))
}

fn raw_to_va(input: &[u8], raw: usize) -> Result<u32> {
    let pe = u32::from_le_bytes(
        input
            .get(0x3c..0x40)
            .ok_or("truncated DOS header")?
            .try_into()
            .unwrap(),
    ) as usize;
    let image_base = u32::from_le_bytes(input[pe + 24 + 28..pe + 24 + 32].try_into().unwrap());
    let count = u16::from_le_bytes(input[pe + 6..pe + 8].try_into().unwrap()) as usize;
    let optional = u16::from_le_bytes(input[pe + 20..pe + 22].try_into().unwrap()) as usize;
    let table = pe + 24 + optional;
    for index in 0..count {
        let section = table + index * 40;
        let relative_va = u32::from_le_bytes(input[section + 12..section + 16].try_into().unwrap());
        let raw_size = u32::from_le_bytes(input[section + 16..section + 20].try_into().unwrap()) as usize;
        let raw_start = u32::from_le_bytes(input[section + 20..section + 24].try_into().unwrap()) as usize;
        if raw >= raw_start && raw < raw_start + raw_size {
            return Ok(image_base + relative_va + (raw - raw_start) as u32);
        }
    }
    Err("title text is outside the executable's mapped sections".into())
}

fn find_bytes(input: &[u8], needle: &[u8]) -> Option<usize> {
    input.windows(needle.len()).position(|window| window == needle)
}

/// Redirects the fixed title-screen `push "Version 1.26"` instructions to
/// the longer credit text. The game already right-aligns the rendered surface,
/// so its right edge stays in the original position without a coordinate patch.
fn patch_title_credit(output: &mut [u8], credit_va: u32) -> Result<usize> {
    let raw = find_bytes(output, TITLE_VERSION).ok_or("could not find the Version 1.26 title text")?;
    let source_va = raw_to_va(output, raw)?;
    let mut changed = 0;
    for offset in 1..output.len().saturating_sub(4) {
        if output[offset - 1] == 0x68
            && u32::from_le_bytes(output[offset..offset + 4].try_into().unwrap()) == source_va
        {
            output[offset..offset + 4].copy_from_slice(&credit_va.to_le_bytes());
            changed += 1;
        }
    }
    if changed != 4 {
        return Err(format!(
            "expected four Version 1.26 title references, found {changed}; executable was not changed"
        ));
    }
    Ok(changed)
}

/// Upgrades a portable EXE made by an older patcher. `.port` already reserves
/// a 4 KiB raw section, so its unused tail is a stable place for the credit.
fn install_title_credit_in_existing_port(output: &mut [u8]) -> Result<bool> {
    if find_bytes(output, TITLE_CREDIT).is_some() {
        return Ok(false);
    }
    let section = find_section(output, b".port")?;
    let pe = u32::from_le_bytes(output[0x3c..0x40].try_into().unwrap()) as usize;
    let image_base = u32::from_le_bytes(output[pe + 24 + 28..pe + 24 + 32].try_into().unwrap());
    let relative_va = u32::from_le_bytes(output[section + 12..section + 16].try_into().unwrap());
    let raw_size = u32::from_le_bytes(output[section + 16..section + 20].try_into().unwrap()) as usize;
    let raw = u32::from_le_bytes(output[section + 20..section + 24].try_into().unwrap()) as usize;
    if raw_size < PORT_TITLE_OFFSET + TITLE_CREDIT.len()
        || raw + PORT_TITLE_OFFSET + TITLE_CREDIT.len() > output.len()
    {
        return Err("existing .port section has no safe room for the title credit".into());
    }
    output[raw + PORT_TITLE_OFFSET..raw + PORT_TITLE_OFFSET + TITLE_CREDIT.len()]
        .copy_from_slice(TITLE_CREDIT);
    // Older patchers set the virtual size to their code length. Extend it so
    // Windows maps the reserved tail where the new text lives.
    output[section + 8..section + 12].copy_from_slice(&(PORT_SIZE as u32).to_le_bytes());
    patch_title_credit(output, image_base + relative_va + PORT_TITLE_OFFSET as u32)?;
    Ok(true)
}

fn discover_font_layout(input: &[u8]) -> Result<FontLayout> {
    let (dseg_va, _, _) = section_location(input, b"DSEG")?;
    let (cseg_va, cseg_raw, cseg_size) = section_location(input, b"CSEG")?;
    if cseg_size < 0x2000 || cseg_raw + 0x2000 > input.len() {
        return Err("CSEG is too small for the verified Vietnamese font hooks".into());
    }
    Ok(FontLayout {
        dseg_va,
        cseg_va,
        cseg_raw,
    })
}

fn font_layout(input: &[u8]) -> Result<FontLayout> {
    let layout = discover_font_layout(input)?;
    for (offset, expected) in [
        (0x01d0, &[0xac, 0x83, 0xc2, 0x10, 0xeb][..]),
        (0x0235, &[0xb8, 0x10, 0x00, 0x00, 0x00][..]),
        (0x02e1, &[0x8a, 0xe0, 0xac, 0x66, 0x25][..]),
        (0x0444, &[0x8a, 0xe0, 0xac, 0x56, 0x66][..]),
    ] {
        expect_bytes(input, layout.cseg_raw + offset, expected).map_err(|_| {
            format!("Vietnamese font hook instruction at CSEG+{offset:#x} is not recognized")
        })?;
    }
    Ok(layout)
}

fn installed_font_layout(input: &[u8]) -> Option<FontLayout> {
    discover_font_layout(input).ok()
}

pub fn has_vietnamese_font_hook(input: &[u8]) -> bool {
    let Some(layout) = installed_font_layout(input) else {
        return false;
    };
    let raw = layout.cseg_raw + 0x01d0;
    let Some(bytes) = input.get(raw..raw + 5) else {
        return false;
    };
    if bytes[0] != 0xe9 {
        return false;
    }
    let relative = i32::from_le_bytes(bytes[1..5].try_into().unwrap()) as i64;
    let target = layout.cseg_va as i64 + 0x01d0 + 5 + relative;
    target == (layout.cseg_va + 0x1c00) as i64
}

fn has_legacy_vietnamese_hook(input: &[u8], layout: FontLayout) -> bool {
    let raw = layout.cseg_raw + 0x11d0;
    let Some(bytes) = input.get(raw..raw + 5) else {
        return false;
    };
    if bytes[0] != 0xe9 {
        return false;
    }
    let relative = i32::from_le_bytes(bytes[1..5].try_into().unwrap()) as i64;
    let target = layout.cseg_va as i64 + 0x11d0 + 5 + relative;
    target == (layout.cseg_va + 0x1c00) as i64
}

pub fn patch_vietnamese(original: &[u8]) -> Result<Vec<u8>> {
    let layout = font_layout(original)?;
    let mut output = original.to_vec();
    if has_legacy_vietnamese_hook(original, layout) {
        for (offset, bytes) in [
            (0x11d0, &[0x05, 0xbb, 0x3f, 0x00, 0x00][..]),
            (0x1235, &[0x88, 0x07, 0x47, 0xe2, 0xed][..]),
            (0x12e1, &[0x01, 0x8b, 0xc1, 0x83, 0xe0][..]),
            (0x1444, &[0x00, 0x45, 0x47, 0x46, 0xe2][..]),
        ] {
            output[layout.cseg_raw + offset..layout.cseg_raw + offset + 5].copy_from_slice(bytes);
        }
    }
    let cave_va = layout.cseg_va + 0x1c00;
    let cave_raw = layout.cseg_raw + 0x1c00;
    let (cave, labels) = vietnamese_cave(cave_va, layout.dseg_va, layout.cseg_va)?;
    if cave.len() > 0x400 {
        return Err(format!(
            "Vietnamese patch uses {}/1024 cave bytes",
            cave.len()
        ));
    }
    output[cave_raw..cave_raw + 0x400].fill(0x90);
    output[cave_raw..cave_raw + cave.len()].copy_from_slice(&cave);
    for (source, label) in [
        (layout.cseg_va + 0x01d0, "measure_string"),
        (layout.cseg_va + 0x0235, "character_width"),
        (layout.cseg_va + 0x02e1, "single_render"),
        (layout.cseg_va + 0x0444, "string_render"),
    ] {
        patch_cseg_jump(
            &mut output,
            layout.cseg_va,
            layout.cseg_raw,
            source,
            labels[label],
        )?;
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
    a.label("title_credit");
    a.emit(TITLE_CREDIT);
    a.label("path_buffer");
    a.emit(&[0_u8; 256]);
    a.finish()
}

fn alternate_no_disc_section() -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
    const IAT_GET_MODULE_FILENAME: u32 = 0x004b_611c;
    const CD_ROOT: u32 = 0x004c_9a78;
    let mut a = Asm::new(PORT_VA);
    a.label("no_disc");
    a.emit(&[0x56, 0x57, 0x68, 0, 1, 0, 0, 0xa1]);
    a.u32(CD_ROOT);
    a.emit(&[0x50, 0x6a, 0]);
    a.call_iat(IAT_GET_MODULE_FILENAME);
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("fallback"));
    a.emit(&[0x8b, 0x3d]);
    a.u32(CD_ROOT);
    a.emit(&[0x01, 0xc7]);
    a.label("scan");
    a.emit(&[0x3b, 0x3d]);
    a.u32(CD_ROOT);
    a.jbe(Target::Label("fallback"));
    a.emit(&[0x4f, 0x80, 0x3f, 0x5c]);
    a.je(Target::Label("found"));
    a.emit(&[0x80, 0x3f, 0x2f]);
    a.jne(Target::Label("scan"));
    a.label("found");
    a.emit(&[0x47, 0xc6, 0x07, 0]);
    a.jmp(Target::Label("done"));
    a.label("fallback");
    a.emit(&[0xa1]);
    a.u32(CD_ROOT);
    a.emit(&[0xc7, 0, 0x2e, 0x5c, 0, 0]);
    a.label("done");
    a.emit(&[0x5f, 0x5e]);
    a.jmp(Target::Address(0x0043_721a));
    a.label("title_credit");
    a.emit(TITLE_CREDIT);
    a.finish()
}

fn expect_bytes(output: &[u8], raw: usize, expected: &[u8]) -> Result<()> {
    if output.get(raw..raw + expected.len()) != Some(expected) {
        return Err(format!("unexpected executable bytes at {raw:#x}"));
    }
    Ok(())
}

fn add_section(input: &[u8], section: &[u8]) -> Result<Vec<u8>> {
    if section.len() > PORT_SIZE {
        return Err(format!(
            "portable section uses {}/{} bytes",
            section.len(),
            PORT_SIZE
        ));
    }
    let port_raw = (input.len() + 0xfff) & !0xfff;
    let mut output = vec![0_u8; port_raw + PORT_SIZE];
    output[..input.len()].copy_from_slice(input);
    output[port_raw..port_raw + section.len()].copy_from_slice(section);
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
        (20, port_raw as u32),
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
    let registry = verified.get(0x2cc11..0x2cc18);
    if registry != Some(&[0x33, 0xc0, 0xe9, 0x35, 0x02, 0, 0])
        && registry != Some(&[0xc7, 0x45, 0xf4, 0, 0, 0, 0])
    {
        return Err("unexpected registry-check bytes in Doraemon.exe".into());
    }
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
    patch_title_credit(&mut output, labels["title_credit"])?;
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

fn patch_alternate_portable(input: &[u8]) -> Result<Vec<u8>> {
    expect_bytes(input, 0x3713b, &[0x68, 0x90, 0xeb, 0x4b, 0x00])?;
    let (registry, _) = patch_registry_at(input, 0x2cb31)?;
    let (section, labels) = alternate_no_disc_section()?;
    let mut output = add_section(&registry, &section)?;
    patch_title_credit(&mut output, labels["title_credit"])?;
    patch_jump(&mut output, 0x0043_713b, labels["no_disc"], 5)?;
    Ok(output)
}

/// Applies only the Setup/registry bypass to an already CD-bypassed build.
/// The existing executable's CD/audio code is deliberately left untouched.
fn patch_registry_at(input: &[u8], offset: usize) -> Result<(Vec<u8>, bool)> {
    const ORIGINAL: &[u8] = &[0x33, 0xc0, 0xe9, 0x35, 0x02, 0, 0];
    const PATCHED: &[u8] = &[0xc7, 0x45, 0xf4, 0, 0, 0, 0];
    if input.get(offset..offset + 7) == Some(PATCHED) {
        return Ok((input.to_vec(), false));
    }
    expect_bytes(input, offset, ORIGINAL)?;
    let mut output = input.to_vec();
    output[offset..offset + 7].copy_from_slice(PATCHED);
    Ok((output, true))
}

pub struct CompatibilityPatch {
    pub bytes: Vec<u8>,
    pub actions: Vec<String>,
    pub local_audio_supported: bool,
}

pub fn patch_language_runtime(
    input: &[u8],
    vietnamese: bool,
    no_disc: bool,
    no_reg: bool,
    local_audio: bool,
) -> Result<CompatibilityPatch> {
    let mut output = input.to_vec();
    let mut actions = Vec::new();
    if vietnamese {
        if has_vietnamese_font_hook(&output) {
            actions.push("Vietnamese font rendering is already installed".into());
        } else {
            output = patch_vietnamese(&output)?;
            actions.push("enabled Vietnamese font rendering".into());
        }
    }
    let compatibility = patch_compatible(&output, no_disc, local_audio, no_reg)?;
    output = compatibility.bytes;
    actions.extend(compatibility.actions);
    Ok(CompatibilityPatch {
        bytes: output,
        actions,
        local_audio_supported: compatibility.local_audio_supported,
    })
}

/// Detects the two v1.26 layouts encountered so far and applies only missing features.
/// Unknown layouts are rejected even when they contain a similar version string.
pub fn patch_compatible(
    input: &[u8],
    no_disc: bool,
    local_audio_requested: bool,
    no_reg: bool,
) -> Result<CompatibilityPatch> {
    let canonical_layout = discover_font_layout(input)
        .map(|layout| layout.cseg_va == CSEG_VA && layout.cseg_raw == CSEG_RAW)
        .unwrap_or(false);
    let alternate_layout = discover_font_layout(input)
        .map(|layout| layout.cseg_va == 0x004c_e000 && layout.cseg_raw == 0x000c_9000)
        .unwrap_or(false);
    let canonical = canonical_layout
        && input
            .get(0x2cc11..0x2cc18)
            .map(|b| {
                b == [0x33, 0xc0, 0xe9, 0x35, 0x02, 0, 0] || b == [0xc7, 0x45, 0xf4, 0, 0, 0, 0]
            })
            .unwrap_or(false);
    let already_portable = find_section(input, b".port").is_ok();
    if already_portable {
        let mut output = input.to_vec();
        let added_credit = install_title_credit_in_existing_port(&mut output)?;
        return Ok(CompatibilityPatch {
            bytes: output,
            actions: if added_credit {
                vec!["added the title-screen patch credit".into()]
            } else {
                Vec::new()
            },
            local_audio_supported: canonical_layout,
        });
    }
    if canonical {
        if no_disc {
            let bytes = patch_portable(input)?;
            return Ok(CompatibilityPatch {
                bytes,
                actions: vec![
                    "added the title-screen patch credit".into(),
                    "bypassed the Setup registry requirement".into(),
                    "bypassed the original CD check".into(),
                    "enabled local DoraemonMusic.wav playback".into(),
                ],
                local_audio_supported: true,
            });
        }
        if !no_reg {
            return Ok(CompatibilityPatch { bytes: input.to_vec(), actions: Vec::new(), local_audio_supported: false });
        }
        let (bytes, changed) = patch_registry_at(input, 0x2cc11)?;
        return Ok(CompatibilityPatch {
            bytes,
            actions: if changed {
                vec!["bypassed the Setup registry requirement".into()]
            } else {
                Vec::new()
            },
            local_audio_supported: false,
        });
    }
    // Alternate December 1998 v1.26 layout. Its CD startup
    // path is already bypassed, but its registry branch is the original one.
    let alternate = alternate_layout
        && input.get(0x3721a..0x37222) == Some(&[0x83, 0x7d, 0xec, 0, 0x74, 0x07, 0x33, 0xc0])
        && input.get(0x83da3..0x83da8) == Some(&[0x8b, 0x55, 0x08, 0x89, 0x55]);
    if alternate {
        if local_audio_requested {
            return Err("this recognized v1.26 layout already bypasses the CD, but its MCI functions differ; local WAV playback is not yet safe for this layout".into());
        }
        let (bytes, changed) = if no_disc {
            (patch_alternate_portable(input)?, true)
        } else if no_reg {
            patch_registry_at(input, 0x2cb31)?
        } else {
            (input.to_vec(), false)
        };
        let mut actions = Vec::new();
        if changed && !no_disc {
            actions.push("bypassed the Setup registry requirement".into());
        }
        if no_disc {
            actions.push("added the title-screen patch credit".into());
            actions.push("bypassed the Setup registry requirement".into());
            actions.push("bypassed the original CD-ROM drive requirement".into());
        }
        return Ok(CompatibilityPatch {
            bytes,
            actions,
            local_audio_supported: false,
        });
    }
    Err("unsupported Doraemon.exe layout; it may be a different release or an unknown modification, so no bytes were changed".into())
}

pub fn build_variants(original: &[u8], vietnamese: bool) -> Result<(Option<Vec<u8>>, Vec<u8>)> {
    // Runtime compatibility is established from PE sections and verified code
    // patterns. A harmless timestamp, checksum, resource, or overlay change
    // must not make an otherwise supported executable unusable.
    font_layout(original)?;
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
    use crate::hash;
    #[test]
    fn rejects_unknown_executable() {
        assert!(build_variants(&[0; 128], false).is_err());
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
            "03b4a1be43d6da93e4b6d98158cce262c2a4fcc3dade8a141d0e5f9fe3cb1ef1"
        );
        assert!(find_bytes(&portable, TITLE_CREDIT).is_some());
        let title_raw = find_bytes(&portable, TITLE_VERSION).unwrap();
        let title_va = raw_to_va(&portable, title_raw).unwrap();
        assert_eq!(
            portable
                .windows(5)
                .filter(|instruction| instruction[0] == 0x68 && instruction[1..] == title_va.to_le_bytes())
                .count(),
            0
        );
        // An EXE patched by the previous portable build did not contain this
        // credit. It must be safely upgradable without restoring first.
        let mut old_portable = portable.clone();
        let credit_raw = find_bytes(&old_portable, TITLE_CREDIT).unwrap();
        let credit_va = raw_to_va(&old_portable, credit_raw).unwrap();
        old_portable[credit_raw..credit_raw + TITLE_CREDIT.len()].fill(0);
        for offset in 1..old_portable.len().saturating_sub(4) {
            if old_portable[offset - 1] == 0x68
                && u32::from_le_bytes(old_portable[offset..offset + 4].try_into().unwrap()) == credit_va
            {
                old_portable[offset..offset + 4].copy_from_slice(&title_va.to_le_bytes());
            }
        }
        let upgraded = patch_compatible(&old_portable, true, false, true).unwrap();
        assert!(find_bytes(&upgraded.bytes, TITLE_CREDIT).is_some());
        assert!(upgraded
            .actions
            .iter()
            .any(|action| action == "added the title-screen patch credit"));
        let (plain_vi, portable_vi) = build_variants(&original, true).unwrap();
        assert_eq!(plain_vi.as_ref().unwrap().len(), original.len());
        assert_eq!(portable_vi.len(), original.len() + PORT_SIZE);
        assert_eq!(&plain_vi.unwrap()[0xcb00a..0xcb016], b"sysfont.dat\0");
    }

    #[test]
    fn recognized_compatibility_layouts_patch_only_missing_features() {
        let (Ok(canonical_path), Ok(december_path)) = (
            std::env::var("DORAEMON_TEST_CANONICAL_EXE"),
            std::env::var("DORAEMON_TEST_DORACHI_EXE"),
        ) else {
            return;
        };
        let canonical = std::fs::read(canonical_path).unwrap();
        let portable = patch_compatible(&canonical, true, false, true).unwrap();
        assert_eq!(portable.bytes.len(), canonical.len() + PORT_SIZE);
        assert!(portable.local_audio_supported);

        let december = std::fs::read(december_path).unwrap();
        let fixed = patch_compatible(&december, true, false, true).unwrap();
        assert_eq!(fixed.bytes.len(), december.len() + PORT_SIZE);
        assert_eq!(
            &fixed.bytes[0x2cb31..0x2cb38],
            &[0xc7, 0x45, 0xf4, 0, 0, 0, 0]
        );
        assert!(!fixed.local_audio_supported);
        assert_eq!(fixed.bytes[0x3713b], 0xe9);
        assert!(find_section(&fixed.bytes, b".port").is_ok());
    }

    #[test]
    fn runtime_vietnamese_patch_handles_both_known_builds() {
        let (Ok(canonical_path), Ok(december_path)) = (
            std::env::var("DORAEMON_TEST_CANONICAL_EXE"),
            std::env::var("DORAEMON_TEST_DORACHI_EXE"),
        ) else {
            return;
        };
        let mut canonical = std::fs::read(&canonical_path).unwrap();
        // A PE timestamp change alters the whole-file hash but not the code
        // structure. Runtime detection must continue to accept it.
        let pe = u32::from_le_bytes(canonical[0x3c..0x40].try_into().unwrap()) as usize;
        canonical[pe + 8] ^= 0x5a;
        let canonical = patch_language_runtime(&canonical, true, true, true, false).unwrap();
        assert!(has_vietnamese_font_hook(&canonical.bytes));
        assert!(canonical.local_audio_supported);

        let mut legacy = std::fs::read(&canonical_path).unwrap();
        let layout = discover_font_layout(&legacy).unwrap();
        for offset in [0x11d0, 0x1235, 0x12e1, 0x1444] {
            let source = layout.cseg_va + offset as u32;
            patch_cseg_jump(
                &mut legacy,
                layout.cseg_va,
                layout.cseg_raw,
                source,
                layout.cseg_va + 0x1c00,
            )
            .unwrap();
        }
        let migrated = patch_language_runtime(&legacy, true, false, true, false).unwrap();
        assert!(has_vietnamese_font_hook(&migrated.bytes));
        assert_eq!(
            &migrated.bytes[layout.cseg_raw + 0x11d0..layout.cseg_raw + 0x11d5],
            &[0x05, 0xbb, 0x3f, 0x00, 0x00]
        );

        let december = std::fs::read(december_path).unwrap();
        let december = patch_language_runtime(&december, true, true, true, false).unwrap();
        assert!(has_vietnamese_font_hook(&december.bytes));
        assert!(!december.local_audio_supported);
    }
}

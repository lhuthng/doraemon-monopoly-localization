//! Executable patch orchestration for the analyzed 1.26 Win32 build.
//!
//! This module locates the original machine-code routines and installs font,
//! no-disc, registry, and optional local-audio hooks. Since the game has no
//! source-level extension point, every patch preserves its calling
//! conventions, object layouts, and untouched PE bytes. The `assembler`
//! submodule emits new caves, while this layer validates known instruction
//! signatures before redirecting existing call sites.

use std::collections::HashMap;

use crate::Result;
mod assembler;
use assembler::{patch_call, patch_cseg_jump, patch_jump, Asm, Target, IMAGE_BASE};

mod bgm_symbols {
    include!(concat!(env!("OUT_DIR"), "/bgm_symbols.rs"));
}
const BGM_RUNTIME: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bgm_runtime.bin"));

const CSEG_VA: u32 = 0x004d_1000;
const CSEG_RAW: usize = 0x000c_c000;
const PORT_VA: u32 = 0x004d_6000;
const PORT_SIZE: usize = 0x4000;
const PORT_TITLE_OFFSET: usize = 0x0c00;
const PORT_VOLUME_OFFSET: usize = 0x0d00;
const PORT_SFX_OFFSET: usize = 0x0f00;
const PORT_BGM_OFFSET: usize = 0x2000;
const TITLE_PREFIX: &[u8] = b"Version ";
const TITLE_SUFFIX: &[u8] = b" - Patched by Thang\0";

/// Routes an encoded byte to the Chinese path or one of the CC/CD Vietnamese
/// two-byte paths. The second byte is consumed only for a recognized prefix.
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

/// Converts a CC/CD pair and the active Vietnamese variant into sysfont slot
/// `640 + variant * 256 + slot`, matching the expanded 1,920-record file.
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

/// Builds the four rendering/measurement entry points used by the Vietnamese
/// runtime patch. It preserves ASCII and Chinese fallbacks byte-for-byte.
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

/// Finds a PE section header by its eight-byte name and returns its file offset.
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

/// Reads a section's virtual address, raw file offset, and raw size.
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

/// Converts a raw file offset into the image VA used by x86 branch fixups.
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
        let raw_size =
            u32::from_le_bytes(input[section + 16..section + 20].try_into().unwrap()) as usize;
        let raw_start =
            u32::from_le_bytes(input[section + 20..section + 24].try_into().unwrap()) as usize;
        if raw >= raw_start && raw < raw_start + raw_size {
            return Ok(image_base + relative_va + (raw - raw_start) as u32);
        }
    }
    Err("title text is outside the executable's mapped sections".into())
}

/// Searches the executable for a literal signature without interpreting data.
fn find_bytes(input: &[u8], needle: &[u8]) -> Option<usize> {
    input
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Finds a printable, NUL-terminated `Version ...` label in the executable.
/// The bounded scan avoids mistaking arbitrary binary data for a title while
/// allowing releases whose version text is not exactly `Version 1.26`.
fn find_version_string(input: &[u8]) -> Option<(usize, usize)> {
    for raw in input
        .windows(TITLE_PREFIX.len())
        .enumerate()
        .filter_map(|(raw, bytes)| (bytes == TITLE_PREFIX).then_some(raw))
    {
        let Some(end) = input[raw + TITLE_PREFIX.len()..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|offset| raw + TITLE_PREFIX.len() + offset)
        else {
            continue;
        };
        let text = &input[raw..end];
        if text.len() > TITLE_PREFIX.len()
            && text.len() <= 64
            && text[TITLE_PREFIX.len()..]
                .iter()
                .all(|byte| (0x20..=0x7e).contains(byte))
        {
            return Some((raw, end - raw));
        }
    }
    None
}

/// Builds the credit string while retaining the original version suffix.
fn title_credit(input: &[u8]) -> Result<Vec<u8>> {
    let (raw, length) =
        find_version_string(input).ok_or("could not find a printable Version title string")?;
    let mut credit = input[raw..raw + length].to_vec();
    credit.extend_from_slice(TITLE_SUFFIX);
    Ok(credit)
}

/// Repoints every title-screen push of the short version string at the longer
/// credit stored in `.port`, keeping the game's existing text placement.
fn patch_title_credit(output: &mut [u8], credit_va: u32) -> Result<usize> {
    let (raw, _) =
        find_version_string(output).ok_or("could not find a printable Version title string")?;
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
            "expected four Version title references, found {changed}; executable was not changed"
        ));
    }
    Ok(changed)
}

/// Upgrades a portable EXE made by an older patcher. `.port` already reserves
/// a 4 KiB raw section, so its unused tail is a stable place for the credit.
/// Migrates an older portable patch in place by refreshing its title text and
/// extending the existing section's mapped tail when necessary.
fn install_title_credit_in_existing_port(output: &mut [u8]) -> Result<bool> {
    let credit = title_credit(output)?;
    let section = find_section(output, b".port")?;
    let pe = u32::from_le_bytes(output[0x3c..0x40].try_into().unwrap()) as usize;
    let image_base = u32::from_le_bytes(output[pe + 24 + 28..pe + 24 + 32].try_into().unwrap());
    let relative_va = u32::from_le_bytes(output[section + 12..section + 16].try_into().unwrap());
    let raw_size =
        u32::from_le_bytes(output[section + 16..section + 20].try_into().unwrap()) as usize;
    let raw = u32::from_le_bytes(output[section + 20..section + 24].try_into().unwrap()) as usize;
    if raw_size < PORT_TITLE_OFFSET + credit.len()
        || raw + PORT_TITLE_OFFSET + credit.len() > output.len()
    {
        return Err("existing .port section has no safe room for the title credit".into());
    }
    let title_range = raw + PORT_TITLE_OFFSET..raw + PORT_TITLE_OFFSET + credit.len();
    let changed = output[title_range.clone()] != credit;
    if changed {
        output[title_range].copy_from_slice(&credit);
    }
    // Older patchers set the virtual size to their code length. Extend it so
    // Windows maps the reserved tail where the new text lives.
    output[section + 8..section + 12].copy_from_slice(&(raw_size as u32).to_le_bytes());
    let (source_raw, _) =
        find_version_string(output).ok_or("could not find a printable Version title string")?;
    let source_va = raw_to_va(output, source_raw)?;
    let source_is_still_referenced = output
        .windows(5)
        .any(|instruction| instruction[0] == 0x68 && instruction[1..] == source_va.to_le_bytes());
    if source_is_still_referenced {
        patch_title_credit(output, image_base + relative_va + PORT_TITLE_OFFSET as u32)?;
    }
    Ok(changed)
}

/// Discovers the canonical CSEG/DSEG locations required by the font cave.
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

/// Returns the exact analyzed executable layout, rejecting unsupported builds.
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

/// Reports whether the Vietnamese renderer's CSEG redirects are already present.
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

/// Detects the earlier font patch so it can be upgraded without stacking caves.
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

/// Adds Vietnamese CC/CD decoding to the game while retaining ASCII/Chinese
/// branches and the original `sysfont.dat` filename.
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

/// Emits the `.port` code/data section shared by no-disc and local-audio modes.
/// Labels are returned so callers can redirect original executable call sites.
fn portable_section(
    modern_sfx: bool,
    credit: &[u8],
) -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
    let mut a = Asm::new(PORT_VA);
    const IAT_MODULE: u32 = 0x004b_90d0;
    const SOUND_MANAGER: u32 = 0x004c_c83c;
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

    a.label("ensure_audio_dispatch");
    a.absolute(&[0xa1], "audio_dispatch");
    a.emit(&[0xc3]);

    a.label("open_music");
    a.call(Target::Label("ensure_audio_dispatch"));
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("audio_open_failed"));
    a.emit(&[0x6a, 0, 0xff, 0x35]);
    a.u32(SOUND_MANAGER);
    a.emit(&[0x6a, 0, 0xff, 0xd0]); // dispatch(INIT, manager, 0)
    a.label("audio_open_failed");
    // This hook replaces the constructor body from 0x485043, not the
    // function entry. The real object pointer was saved at [ebp-0x70] before
    // the hook point; ECX was clobbered by the preceding memset call.
    a.emit(&[0x8b, 0x55, 0x90]); // mov edx,[ebp-0x70]
    a.emit(&[0xc6, 0x02, 0xff]); // current track = none
    a.emit(&[0x66, 0xc7, 0x42, 0x02, 0x01, 0x00]); // harmless local device id
    a.emit(&[0x89, 0x42, 0x08]); // open result
    a.emit(&[0xc7, 0x42, 0x0c, 0, 0, 0, 0]);
    a.emit(&[0xc7, 0x42, 0x14, 0, 0, 0, 0]);
    a.emit(&[0xc7, 0x42, 0x18, 0, 0, 0, 0]);
    a.emit(&[0x89, 0xd0, 0x8b, 0xe5, 0x5d, 0xc2, 0x04, 0x00]);

    a.label("close_music");
    a.call(Target::Label("ensure_audio_dispatch"));
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("close_music_done"));
    a.emit(&[0x6a, 0, 0x6a, 0, 0x6a, 0x06, 0xff, 0xd0]);
    a.label("close_music_done");
    a.emit(&[0xc3]);

    a.label("play");
    a.emit(&[0x55, 0x8b, 0xec, 0x56, 0x89, 0xce, 0x83, 0x7e, 0x10, 0]);
    a.je(Target::Label("play_reject"));
    a.call(Target::Label("ensure_audio_dispatch"));
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("play_reject"));
    a.emit(&[0x8b, 0x55, 0x08, 0x81, 0xe2, 0xff, 0x00, 0x00, 0x00]);
    a.emit(&[0xff, 0x75, 0x0c, 0x52, 0x6a, 0x01, 0xff, 0xd0]);
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("play_reject"));
    a.emit(&[0x8b, 0x55, 0x08, 0x88, 0x16, 0x83, 0x7d, 0x0c, 0]);
    a.je(Target::Label("play_success"));
    a.call(Target::Label("ensure_audio_dispatch"));
    a.emit(&[0x6a, 0, 0xff, 0x75, 0x08, 0x6a, 0x03, 0xff, 0xd0]);
    a.emit(&[0x89, 0x46, 0x14, 0xc7, 0x46, 0x0c, 1, 0, 0, 0]);
    a.label("play_success");
    a.emit(&[0x31, 0xc0, 0x5e, 0x8b, 0xe5, 0x5d, 0xc2, 0x08, 0]);
    a.label("play_reject");
    a.emit(&[0xb8, 1, 0, 0, 0, 0x5e, 0x8b, 0xe5, 0x5d, 0xc2, 0x08, 0]);

    a.label("stop");
    a.emit(&[0x56, 0x89, 0xce]);
    a.call(Target::Label("ensure_audio_dispatch"));
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("stop_done"));
    a.emit(&[0x6a, 0, 0x6a, 0, 0x6a, 0x02, 0xff, 0xd0]);
    a.label("stop_done");
    a.emit(&[0xc7, 0x46, 0x0c, 0, 0, 0, 0, 0x5e, 0xc3]);

    a.label("duration");
    a.call(Target::Label("ensure_audio_dispatch"));
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("duration_zero"));
    a.emit(&[
        0x6a, 0, 0xff, 0x74, 0x24, 0x08, 0x6a, 0x03, 0xff, 0xd0, 0xc2, 0x04, 0,
    ]);
    a.label("duration_zero");
    a.emit(&[0x31, 0xc0, 0xc2, 0x04, 0]);
    a.label("track_count");
    a.emit(&[0xb8, 0x0b, 0, 0, 0, 0xc3]);
    a.label("audio_dispatch");
    a.u32(bgm_symbols::BGMDISPATCH);
    a.label("title_credit");
    a.emit(credit);
    a.label("path_buffer");
    a.emit(&[0_u8; 256]);
    if a.bytes.len() > PORT_VOLUME_OFFSET {
        return Err("portable core overlaps the reserved local-volume block".into());
    }
    while a.bytes.len() < PORT_VOLUME_OFFSET {
        a.emit(&[0x90]);
    }
    let (volume, volume_labels) = local_music_volume_block(
        PORT_VA + PORT_VOLUME_OFFSET as u32,
        a.labels["audio_dispatch"],
    )?;
    if volume.len() > PORT_SFX_OFFSET - PORT_VOLUME_OFFSET {
        return Err("local music volume hook does not fit in the reserved .port section".into());
    }
    a.labels.extend(volume_labels);
    a.emit(&volume);
    if modern_sfx {
        if a.bytes.len() > PORT_SFX_OFFSET {
            return Err("local-volume block overlaps the reserved SFX block".into());
        }
        while a.bytes.len() < PORT_SFX_OFFSET {
            a.emit(&[0x90]);
        }
        let (sfx, sfx_labels) = portable_sfx_volume_block(PORT_VA + PORT_SFX_OFFSET as u32)?;
        if sfx.len() > PORT_SIZE - PORT_SFX_OFFSET {
            return Err("modern SFX volume hook does not fit in the reserved .port section".into());
        }
        a.labels.extend(sfx_labels);
        a.emit(&sfx);
    }
    if a.bytes.len() > PORT_BGM_OFFSET {
        return Err("portable hooks overlap the embedded BGM runtime".into());
    }
    while a.bytes.len() < PORT_BGM_OFFSET {
        a.emit(&[0]);
    }
    if BGM_RUNTIME.len() > PORT_SIZE - PORT_BGM_OFFSET {
        return Err("embedded BGM runtime does not fit in .port".into());
    }
    a.emit(BGM_RUNTIME);
    a.finish()
}

/// For local music only, forwards the game's BGM slider value to the embedded
/// DirectSound stream. The normal CD/MCI path never points at this block.
/// Emits the Music-slider bridge. It forwards the game's 0..65535 value to
/// BgmDispatch command 5 and reports full width when no legacy mixer exists.
fn local_music_volume_block(
    base: u32,
    dispatch_pointer: u32,
) -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
    let mut a = Asm::new(base);
    a.label("wave_bgm_get_volume");
    a.emit(&[0xa1]);
    a.u32(dispatch_pointer);
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("wave_bgm_default_volume"));
    a.emit(&[0x6a, 0x00, 0x6a, 0x00, 0x6a, 0x07, 0xff, 0xd0]);
    a.jmp(Target::Label("wave_bgm_store_volume"));
    a.label("wave_bgm_default_volume");
    a.emit(&[0xb8]);
    a.u32(0xffff);
    a.label("wave_bgm_store_volume");
    a.emit(&[0x89, 0xc2]);
    a.emit(&[0x8b, 0x44, 0x24, 0x08]); // descriptor
    a.emit(&[0x8b, 0x40, 0x14]); // descriptor->paDetails
    a.emit(&[0x89, 0x10, 0x89, 0x50, 0x04]);
    a.emit(&[0x31, 0xc0, 0xc2, 0x0c, 0x00]); // success; stdcall return

    a.label("wave_bgm_volume");
    a.emit(&[0x60]); // pushad
    a.emit(&[0xa1]);
    a.u32(dispatch_pointer);
    a.emit(&[0x85, 0xc0]);
    a.je(Target::Label("wave_volume_done"));
    // The original handler's final 16-bit 0..65535 value lives at
    // [ebp-0x2c]. DoraAudioDispatch command 5 maps it to DirectSound dB.
    a.emit(&[0x0f, 0xb7, 0x55, 0xd4]);
    a.emit(&[0x6a, 0x00, 0x52, 0x6a, 0x05, 0xff, 0xd0]);
    a.label("wave_volume_done");
    a.emit(&[0x61]); // popad
    a.jmp(Target::Address(0x0048_b466));
    a.finish()
}

/// Replaces the ignored legacy WaveOut mixer control with the equivalent
/// DirectSound-buffer attenuation. The table is logarithmic: 50% is roughly
/// -6 dB, while zero is muted, matching a conventional modern volume slider.
/// Emits the optional modern SFX mixer bridge; it is independent of local music.
fn portable_sfx_volume_block(base: u32) -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
    let mut a = Asm::new(base);
    // The old mixer read returns unusable values under CrossOver even though
    // new DirectSound buffers begin at full volume. Report the tracked level
    // and provide the 16-bit range used by the game's initial knob-position
    // calculation. Subsequent writes update both values below.
    a.label("sfx_get_volume");
    a.emit(&[0x8b, 0x4c, 0x24, 0x08]); // descriptor
    a.emit(&[0x8b, 0x41, 0x14]); // descriptor->paDetails
    a.absolute(&[0x8b, 0x15], "sfx_current_level");
    a.emit(&[0x89, 0x10, 0x89, 0x50, 0x04]);
    // descriptor is object+0x19c, therefore +0x80 is object+0x21c.
    a.emit(&[0xc7, 0x81, 0x80, 0x00, 0x00, 0x00]);
    a.u32(0xffff);
    a.emit(&[0x31, 0xc0, 0xc2, 0x0c, 0x00]); // success; stdcall return

    a.label("sfx_slider_volume");
    a.emit(&[0x60]); // pushad
    a.emit(&[0x0f, 0xb7, 0x45, 0xe8]); // UI value, 0..65535
    a.absolute(&[0xa3], "sfx_current_level");
    a.absolute(&[0xa1], "sfx_current_level");
    a.emit(&[0x69, 0xc0]);
    a.u32(212);
    a.emit(&[0x31, 0xd2, 0xb9]);
    a.u32(65_535);
    a.emit(&[0xf7, 0xf1, 0xc1, 0xe8, 0x02, 0x8b, 0x04, 0x85]); // div; /4; table lookup
    a.fix(&[], Target::Label("sfx_volume_table"), false);
    a.absolute(&[0xa3], "sfx_master_attenuation");
    a.emit(&[0x61, 0xc2, 0x0c, 0x00]); // popad; ret 12

    a.label("sfx_set_volume_1");
    a.absolute(&[0xc7, 0x04, 0x24], "sfx_set_volume_return_1");
    a.jmp(Target::Label("sfx_set_volume_common"));
    a.label("sfx_set_volume_2");
    a.absolute(&[0xc7, 0x04, 0x24], "sfx_set_volume_return_2");
    a.label("sfx_set_volume_common");
    a.absolute(&[0xa1], "sfx_master_attenuation");
    a.emit(&[0x01, 0x44, 0x24, 0x08]); // add [esp+8],eax
    a.emit(&[0x81, 0x7c, 0x24, 0x08]);
    a.u32((-10_000_i32) as u32);
    a.jge(Target::Label("sfx_set_volume_forward"));
    a.emit(&[0xc7, 0x44, 0x24, 0x08]);
    a.u32((-10_000_i32) as u32);
    a.label("sfx_set_volume_forward");
    a.emit(&[0x8b, 0x44, 0x24, 0x04, 0x8b, 0x00, 0xff, 0x60, 0x3c]);
    a.label("sfx_set_volume_return_1");
    a.emit(&[0x6a, 0x00]);
    a.jmp(Target::Address(0x0048_921d));
    a.label("sfx_set_volume_return_2");
    a.emit(&[0x6a, 0x00]);
    a.jmp(Target::Address(0x0048_9519));

    a.label("sfx_master_attenuation");
    a.u32(0);
    a.label("sfx_current_level");
    a.u32(0xffff);
    a.label("sfx_volume_table");
    for step in 0..=53 {
        let ratio = (step * 4).min(212) as f64 / 212.0;
        let attenuation = if step == 0 {
            -10_000
        } else {
            (2_000.0 * ratio.log10()).round() as i32
        }
        .clamp(-10_000, 0);
        a.u32(attenuation as u32);
    }
    a.finish()
}

/// Builds the no-disc cave for the alternate 1.26 executable layout.
fn alternate_no_disc_section(credit: &[u8]) -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
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
    a.emit(credit);
    a.finish()
}

/// Guards a patch site against silently modifying an unknown executable build.
fn expect_bytes(output: &[u8], raw: usize, expected: &[u8]) -> Result<()> {
    if output.get(raw..raw + expected.len()) != Some(expected) {
        return Err(format!("unexpected executable bytes at {raw:#x}"));
    }
    Ok(())
}

/// Appends a mapped executable section while preserving all original sections.
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
    output[pe + 24 + 56..pe + 24 + 60]
        .copy_from_slice(&(PORT_VA - IMAGE_BASE + PORT_SIZE as u32).to_le_bytes());
    output[pe + 24 + 64..pe + 24 + 68].fill(0);
    Ok(output)
}

/// Applies the requested compatibility options to the canonical executable.
/// Each option is independently guarded so already-applied options remain safe.
fn patch_portable_options(
    verified: &[u8],
    no_disc: bool,
    no_reg: bool,
    local_audio: bool,
    modern_volume: bool,
) -> Result<Vec<u8>> {
    let registry = verified.get(0x2cc11..0x2cc18);
    if registry != Some(&[0x33, 0xc0, 0xe9, 0x35, 0x02, 0, 0])
        && registry != Some(&[0xc7, 0x45, 0xf4, 0, 0, 0, 0])
    {
        return Err("unexpected registry-check bytes in Doraemon.exe".into());
    }
    if no_disc {
        expect_bytes(verified, 0x3723a, &[0xff, 0x15, 0xac, 0x90, 0x4b, 0])?;
    }
    for (raw, bytes) in [
        (0x85043, &[0x8b, 0x55, 0x08, 0x89, 0x55][..]),
        (0x851d9, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
        (0x85288, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
        (0x85366, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
        (0x8545f, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
        (0x855f3, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
    ] {
        expect_bytes(verified, raw, bytes)?;
    }
    let credit = title_credit(verified)?;
    let (section, labels) = portable_section(modern_volume, &credit)?;
    let mut output = add_section(verified, &section)?;
    patch_title_credit(&mut output, labels["title_credit"])?;
    if modern_volume {
        install_modern_sfx_volume_hook(&mut output, &labels)?;
    }
    if local_audio {
        install_local_music_bgm_volume_hook(
            &mut output,
            labels["wave_bgm_volume"],
            labels["wave_bgm_get_volume"],
        )?;
    }
    if no_reg {
        output[0x2cc11..0x2cc18].copy_from_slice(&[0xc7, 0x45, 0xf4, 0, 0, 0, 0]);
    }
    if no_disc {
        patch_jump(&mut output, 0x0043_723a, labels["no_disc"], 6)?;
    }
    if local_audio {
        for (source, label, replaced) in [
            (0x0048_5043, "open_music", 5),
            (0x0048_51d9, "close_music", 5),
            (0x0048_5288, "play", 5),
            (0x0048_5366, "stop", 5),
            (0x0048_545f, "duration", 5),
            (0x0048_55f3, "track_count", 5),
        ] {
            patch_jump(&mut output, source, labels[label], replaced)?;
        }
    }
    Ok(output)
}

/// Convenience wrapper for the default no-disc portable configuration.
pub fn patch_portable(verified: &[u8]) -> Result<Vec<u8>> {
    patch_portable_options(verified, true, true, true, false)
}

/// Redirects the two legacy SFX mixer call sites to the generated DirectSound
/// implementation and updates the initial knob position.
fn install_modern_sfx_volume_hook(
    output: &mut [u8],
    labels: &HashMap<&'static str, u32>,
) -> Result<bool> {
    const LEGACY_CONTROL_GUARD_OFFSET: usize = 0x8b334;
    const LEGACY_CONTROL_GUARD: [u8; 6] = [0x0f, 0x84, 0xb2, 0x00, 0x00, 0x00];
    const MODERN_CONTROL_GUARD: [u8; 6] = [0x90; 6];
    const LEGACY_RANGE_OFFSET: usize = 0x8b351;
    const LEGACY_RANGE: [u8; 7] = [0x0f, 0xaf, 0x81, 0x1c, 0x02, 0x00, 0x00];
    const MODERN_RANGE: [u8; 7] = [0x69, 0xc0, 0xff, 0xff, 0x00, 0x00, 0x90];
    const INITIAL_POSITION_OFFSET: usize = 0x8a277;
    const LEGACY_INITIAL_POSITION: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
    const MODERN_INITIAL_POSITION: [u8; 4] = [0xd4, 0x00, 0x00, 0x00];
    let mut changed = false;
    match output.get(LEGACY_CONTROL_GUARD_OFFSET..LEGACY_CONTROL_GUARD_OFFSET + 6) {
        Some(bytes) if bytes == LEGACY_CONTROL_GUARD => {
            output[LEGACY_CONTROL_GUARD_OFFSET..LEGACY_CONTROL_GUARD_OFFSET + 6]
                .copy_from_slice(&MODERN_CONTROL_GUARD);
            changed = true;
        }
        Some(bytes) if bytes == MODERN_CONTROL_GUARD => {}
        _ => return Err("could not locate the verified legacy SFX mixer guard".into()),
    }
    match output.get(LEGACY_RANGE_OFFSET..LEGACY_RANGE_OFFSET + LEGACY_RANGE.len()) {
        Some(bytes) if bytes == LEGACY_RANGE => {
            output[LEGACY_RANGE_OFFSET..LEGACY_RANGE_OFFSET + MODERN_RANGE.len()]
                .copy_from_slice(&MODERN_RANGE);
            changed = true;
        }
        Some(bytes) if bytes == MODERN_RANGE => {}
        _ => return Err("could not locate the verified legacy SFX volume range".into()),
    }
    match output.get(INITIAL_POSITION_OFFSET..INITIAL_POSITION_OFFSET + 4) {
        Some(bytes) if bytes == LEGACY_INITIAL_POSITION => {
            output[INITIAL_POSITION_OFFSET..INITIAL_POSITION_OFFSET + 4]
                .copy_from_slice(&MODERN_INITIAL_POSITION);
            changed = true;
        }
        Some(bytes) if bytes == MODERN_INITIAL_POSITION => {}
        _ => return Err("could not locate the verified initial SFX slider position".into()),
    }
    for (va, original, target) in [
        (
            0x0048_a162,
            &[0xff, 0x15, 0x78, 0x92, 0x4b, 0x00][..],
            labels["sfx_get_volume"],
        ),
        (
            0x0048_b3b0,
            &[0xff, 0x15, 0x68, 0x92, 0x4b, 0x00][..],
            labels["sfx_slider_volume"],
        ),
        (
            0x0048_9218,
            &[0xff, 0x52, 0x3c, 0x6a, 0x00][..],
            labels["sfx_set_volume_1"],
        ),
        (
            0x0048_9514,
            &[0xff, 0x51, 0x3c, 0x6a, 0x00][..],
            labels["sfx_set_volume_2"],
        ),
    ] {
        let raw = (va - IMAGE_BASE) as usize;
        let current = output
            .get(raw..raw + original.len())
            .ok_or("SFX-volume instruction lies outside Doraemon.exe")?;
        let expected = target.wrapping_sub(va + 5).to_le_bytes();
        if current[0] == 0xe8 && current[1..5] == expected {
            continue;
        }
        let previous_hook = current[0] == 0xe8 && {
            let displacement = i32::from_le_bytes(current[1..5].try_into().unwrap());
            let destination = (va + 5).wrapping_add(displacement as u32);
            (PORT_VA + PORT_SFX_OFFSET as u32..PORT_VA + PORT_SIZE as u32).contains(&destination)
        };
        if current != original && !previous_hook {
            return Err(format!(
                "could not locate verified SFX-volume instruction at {va:#x}"
            ));
        }
        patch_call(output, va, target, original.len())?;
        changed = true;
    }
    Ok(changed)
}

/// The original options screen sends BGM changes to the Compact Disc mixer
/// control. The local-music build keeps its value calculation but forwards the
/// result to the injected runtime's DirectSound buffer. These hooks are installed only
/// for local BGM.dat playback; normal CD-audio builds retain the exact
/// original calls.
/// Redirects the legacy Music mixer calls to local DirectSound volume control,
/// including the constructor fallback that controls the initial Music bell.
fn install_local_music_bgm_volume_hook(
    output: &mut [u8],
    write_target: u32,
    read_target: u32,
) -> Result<bool> {
    const LEGACY_CONTROL_GUARD_OFFSET: usize = 0x8b402;
    const LEGACY_CONTROL_GUARD: [u8; 2] = [0x74, 0x7c];
    const LOCAL_CONTROL_GUARD: [u8; 2] = [0x90, 0x90];
    const LEGACY_RANGE_OFFSET: usize = 0x8b41b;
    const LEGACY_RANGE: [u8; 7] = [0x0f, 0xaf, 0x82, 0x20, 0x02, 0x00, 0x00];
    const LOCAL_RANGE: [u8; 7] = [0x69, 0xc0, 0xff, 0xff, 0x00, 0x00, 0x90];
    const INITIAL_POSITION_OFFSET: usize = 0x8a3eb;
    const LEGACY_INITIAL_POSITION: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
    const LOCAL_INITIAL_POSITION: [u8; 4] = [0xd4, 0x00, 0x00, 0x00];
    const CONTROL_DETAILS_OFFSET: usize = 0x8b458;
    const CALL_OFFSET: usize = 0x8b460;
    const SOURCE_VA: u32 = 0x0048_b460;
    const CD_CONTROL: [u8; 5] = [0x05, 0xb4, 0x01, 0x00, 0x00];
    const WAVE_CONTROL: [u8; 5] = [0x05, 0x9c, 0x01, 0x00, 0x00];
    const ORIGINAL_CALL: [u8; 6] = [0xff, 0x15, 0x68, 0x92, 0x4b, 0x00];
    let mut changed = false;
    match output.get(LEGACY_CONTROL_GUARD_OFFSET..LEGACY_CONTROL_GUARD_OFFSET + 2) {
        Some(bytes) if bytes == LEGACY_CONTROL_GUARD => {
            output[LEGACY_CONTROL_GUARD_OFFSET..LEGACY_CONTROL_GUARD_OFFSET + 2]
                .copy_from_slice(&LOCAL_CONTROL_GUARD);
            changed = true;
        }
        Some(bytes) if bytes == LOCAL_CONTROL_GUARD => {}
        _ => return Err("could not locate the verified legacy BGM mixer guard".into()),
    }
    match output.get(LEGACY_RANGE_OFFSET..LEGACY_RANGE_OFFSET + LEGACY_RANGE.len()) {
        Some(bytes) if bytes == LEGACY_RANGE => {
            output[LEGACY_RANGE_OFFSET..LEGACY_RANGE_OFFSET + LOCAL_RANGE.len()]
                .copy_from_slice(&LOCAL_RANGE);
            changed = true;
        }
        Some(bytes) if bytes == LOCAL_RANGE => {}
        _ => return Err("could not locate the verified legacy BGM volume range".into()),
    }
    match output.get(INITIAL_POSITION_OFFSET..INITIAL_POSITION_OFFSET + 4) {
        Some(bytes) if bytes == LEGACY_INITIAL_POSITION => {
            output[INITIAL_POSITION_OFFSET..INITIAL_POSITION_OFFSET + 4]
                .copy_from_slice(&LOCAL_INITIAL_POSITION);
            changed = true;
        }
        Some(bytes) if bytes == LOCAL_INITIAL_POSITION => {}
        _ => return Err("could not locate the verified initial Music slider position".into()),
    }
    match output.get(CONTROL_DETAILS_OFFSET..CONTROL_DETAILS_OFFSET + 5) {
        Some(bytes) if bytes == CD_CONTROL => {}
        Some(bytes) if bytes == WAVE_CONTROL => {
            output[CONTROL_DETAILS_OFFSET..CONTROL_DETAILS_OFFSET + 5].copy_from_slice(&CD_CONTROL);
            changed = true;
        }
        _ => return Err("could not locate the verified Compact Disc BGM-volume control".into()),
    }
    let expected = write_target.wrapping_sub(SOURCE_VA + 5).to_le_bytes();
    let current = output
        .get(CALL_OFFSET..CALL_OFFSET + 6)
        .ok_or("BGM-volume call lies outside Doraemon.exe")?;
    if current == ORIGINAL_CALL {
        patch_jump(output, SOURCE_VA, write_target, 6)?;
        changed = true;
    } else if !(current[0] == 0xe9 && current[1..5] == expected && current[5] == 0x90) {
        let previous_hook = current[0] == 0xe9 && current[5] == 0x90 && {
            let displacement = i32::from_le_bytes(current[1..5].try_into().unwrap());
            let destination = (SOURCE_VA + 5).wrapping_add(displacement as u32);
            (PORT_VA + PORT_VOLUME_OFFSET as u32..PORT_VA + PORT_SFX_OFFSET as u32)
                .contains(&destination)
        };
        if previous_hook {
            patch_jump(output, SOURCE_VA, write_target, 6)?;
            changed = true;
        } else {
            return Err("could not locate the verified Compact Disc BGM-volume call".into());
        }
    }

    const READ_VA: u32 = 0x0048_9f71;
    const READ_OFFSET: usize = 0x89f71;
    const ORIGINAL_READ: [u8; 6] = [0xff, 0x15, 0x78, 0x92, 0x4b, 0x00];
    let expected_read = read_target.wrapping_sub(READ_VA + 5).to_le_bytes();
    let current_read = output
        .get(READ_OFFSET..READ_OFFSET + ORIGINAL_READ.len())
        .ok_or("BGM-volume read lies outside Doraemon.exe")?;
    if !(current_read[0] == 0xe8 && current_read[1..5] == expected_read) {
        if current_read != ORIGINAL_READ {
            return Err("could not locate the verified Compact Disc BGM-volume read".into());
        }
        patch_call(output, READ_VA, read_target, ORIGINAL_READ.len())?;
        changed = true;
    }
    Ok(changed)
}

/// Upgrades an older `.port` patch with the current BGM volume hook in place.
fn install_local_music_volume_upgrade(output: &mut [u8]) -> Result<bool> {
    if find_bytes(output, b"BGMRT3\0").is_some() {
        let (_, labels) = portable_section(false, b"\0")?;
        return install_local_music_bgm_volume_hook(
            output,
            labels["wave_bgm_volume"],
            labels["wave_bgm_get_volume"],
        );
    }
    let section = find_section(output, b".port")?;
    let raw_size =
        u32::from_le_bytes(output[section + 16..section + 20].try_into().unwrap()) as usize;
    let raw = u32::from_le_bytes(output[section + 20..section + 24].try_into().unwrap()) as usize;
    let dll_name = find_bytes(output, b"doraudio.dll\0")
        .ok_or("this older portable build uses the retired local-WAV backend; restore the original executable before applying the new local-music patch")?;
    let dispatch_pointer = raw_to_va(output, dll_name)?
        .checked_sub(4)
        .ok_or("invalid local-audio dispatcher location")?;
    let (block, labels) =
        local_music_volume_block(PORT_VA + PORT_VOLUME_OFFSET as u32, dispatch_pointer)?;
    if raw_size < PORT_VOLUME_OFFSET + block.len()
        || raw + PORT_VOLUME_OFFSET + block.len() > output.len()
    {
        return Err(
            "existing .port section has no safe room for the local music volume hook".into(),
        );
    }
    output[raw + PORT_VOLUME_OFFSET..raw + PORT_VOLUME_OFFSET + block.len()]
        .copy_from_slice(&block);
    output[section + 8..section + 12].copy_from_slice(&(raw_size as u32).to_le_bytes());
    install_local_music_bgm_volume_hook(
        output,
        labels["wave_bgm_volume"],
        labels["wave_bgm_get_volume"],
    )
}

/// Upgrades an already portable executable to local BGM.dat playback.
fn install_local_music_runtime_upgrade(output: &mut [u8]) -> Result<bool> {
    if find_bytes(output, b"doraudio.dll\0").is_some() {
        return Err("this executable uses the retired doraudio.dll/Music.dat backend; restore the original executable before enabling BGM.dat".into());
    }
    if find_bytes(output, b"BGMRT3\0").is_none() {
        return Err("this older portable build predates embedded BGM.dat streaming; restore the original executable before enabling local music".into());
    }
    let (_, labels) = portable_section(false, b"\0")?;
    let mut changed = false;
    for (raw, original, source, label) in [
        (
            0x85043,
            &[0x8b, 0x55, 0x08, 0x89, 0x55][..],
            0x0048_5043,
            "open_music",
        ),
        (
            0x851d9,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_51d9,
            "close_music",
        ),
        (
            0x85288,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_5288,
            "play",
        ),
        (
            0x85366,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_5366,
            "stop",
        ),
        (
            0x8545f,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_545f,
            "duration",
        ),
        (
            0x855f3,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_55f3,
            "track_count",
        ),
    ] {
        let current = output
            .get(raw..raw + original.len())
            .ok_or("local-music instruction lies outside Doraemon.exe")?;
        let expected = labels[label].wrapping_sub(source + 5).to_le_bytes();
        if current[0] == 0xe9 && current[1..5] == expected {
            continue;
        }
        if current != original {
            return Err(format!(
                "could not locate verified local-music instruction at {source:#x}"
            ));
        }
        patch_jump(output, source, labels[label], original.len())?;
        changed = true;
    }
    if install_local_music_volume_upgrade(output)? {
        changed = true;
    }
    Ok(changed)
}

/// Restores the original CD/MCI Music routines and constructor fallback.
fn disable_local_music_runtime(output: &mut [u8]) -> Result<bool> {
    if find_bytes(output, b"BGMRT3\0").is_none() || output.get(0x85043) != Some(&0xe9) {
        return Ok(false);
    }
    let (_, labels) = portable_section(false, b"\0")?;
    for (raw, original, source, label) in [
        (
            0x85043,
            &[0x8b, 0x55, 0x08, 0x89, 0x55][..],
            0x0048_5043,
            "open_music",
        ),
        (
            0x851d9,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_51d9,
            "close_music",
        ),
        (
            0x85288,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_5288,
            "play",
        ),
        (
            0x85366,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_5366,
            "stop",
        ),
        (
            0x8545f,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_545f,
            "duration",
        ),
        (
            0x855f3,
            &[0x55, 0x8b, 0xec, 0x83, 0xec][..],
            0x0048_55f3,
            "track_count",
        ),
    ] {
        let current = output
            .get(raw..raw + original.len())
            .ok_or("local-music instruction lies outside Doraemon.exe")?;
        let expected = labels[label].wrapping_sub(source + 5).to_le_bytes();
        if current[0] != 0xe9 || current[1..5] != expected {
            return Err(format!(
                "could not safely disable local-music hook at {source:#x}"
            ));
        }
        output[raw..raw + original.len()].copy_from_slice(original);
    }
    output[0x8b460..0x8b466].copy_from_slice(&[0xff, 0x15, 0x68, 0x92, 0x4b, 0x00]);
    output[0x89f71..0x89f77].copy_from_slice(&[0xff, 0x15, 0x78, 0x92, 0x4b, 0x00]);
    output[0x8b402..0x8b404].copy_from_slice(&[0x74, 0x7c]);
    output[0x8b41b..0x8b422].copy_from_slice(&[0x0f, 0xaf, 0x82, 0x20, 0x02, 0x00, 0x00]);
    output[0x8a3eb..0x8a3ef].copy_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    Ok(true)
}

/// Installs modern SFX volume support into an existing `.port` section.
fn install_modern_sfx_volume_upgrade(output: &mut [u8]) -> Result<bool> {
    let section = find_section(output, b".port")?;
    let raw_size =
        u32::from_le_bytes(output[section + 16..section + 20].try_into().unwrap()) as usize;
    let raw = u32::from_le_bytes(output[section + 20..section + 24].try_into().unwrap()) as usize;
    let (block, labels) = portable_sfx_volume_block(PORT_VA + PORT_SFX_OFFSET as u32)?;
    if raw_size < PORT_SFX_OFFSET + block.len()
        || raw + PORT_SFX_OFFSET + block.len() > output.len()
    {
        return Err("existing .port section has no safe room for modern SFX volume control".into());
    }
    output[raw + PORT_SFX_OFFSET..raw + PORT_SFX_OFFSET + block.len()].copy_from_slice(&block);
    output[section + 8..section + 12].copy_from_slice(&(raw_size as u32).to_le_bytes());
    install_modern_sfx_volume_hook(output, &labels)
}

/// Applies no-disc behavior to the recognized alternate executable layout.
fn patch_alternate_portable(input: &[u8]) -> Result<Vec<u8>> {
    expect_bytes(input, 0x3713b, &[0x68, 0x90, 0xeb, 0x4b, 0x00])?;
    let (registry, _) = patch_registry_at(input, 0x2cb31)?;
    let credit = title_credit(input)?;
    let (section, labels) = alternate_no_disc_section(&credit)?;
    let mut output = add_section(&registry, &section)?;
    patch_title_credit(&mut output, labels["title_credit"])?;
    patch_jump(&mut output, 0x0043_713b, labels["no_disc"], 5)?;
    Ok(output)
}

/// Applies only the Setup/registry bypass to an already CD-bypassed build.
/// The existing executable's CD/audio code is deliberately left untouched.
/// NOPs the Setup registry gate at a layout-specific verified offset.
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

/// Combines Vietnamese font and compatibility edits for a selected language.
pub fn patch_language_runtime(
    input: &[u8],
    vietnamese: bool,
    no_disc: bool,
    no_reg: bool,
    local_audio: bool,
    modern_volume: bool,
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
    let compatibility = patch_compatible(&output, no_disc, local_audio, no_reg, modern_volume)?;
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
/// Detects the executable layout and applies only requested missing features.
pub fn patch_compatible(
    input: &[u8],
    no_disc: bool,
    local_audio_requested: bool,
    no_reg: bool,
    modern_volume: bool,
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
        let upgraded_local_music = if canonical_layout && local_audio_requested {
            install_local_music_runtime_upgrade(&mut output)?
        } else {
            false
        };
        let disabled_local_music = if canonical_layout && !local_audio_requested {
            disable_local_music_runtime(&mut output)?
        } else {
            false
        };
        let upgraded_sfx_volume = if canonical_layout && modern_volume {
            install_modern_sfx_volume_upgrade(&mut output)?
        } else {
            false
        };
        let mut actions = Vec::new();
        if added_credit {
            actions.push("added the title-screen patch credit".into());
        }
        if upgraded_local_music {
            actions.push(
                "enabled embedded BGM.dat streaming and its DirectSound volume control".into(),
            );
        }
        if disabled_local_music {
            actions.push("restored the original CD/MCI music and volume routines".into());
        }
        if upgraded_sfx_volume {
            actions.push("enabled modern SFX volume control".into());
        }
        return Ok(CompatibilityPatch {
            bytes: output,
            actions,
            local_audio_supported: canonical_layout,
        });
    }
    if canonical {
        if no_disc || local_audio_requested || modern_volume {
            let bytes = patch_portable_options(
                input,
                no_disc,
                no_reg,
                local_audio_requested,
                modern_volume,
            )?;
            let mut actions = vec!["added the title-screen patch credit".into()];
            if no_reg {
                actions.push("bypassed the Setup registry requirement".into());
            }
            if no_disc {
                actions.push("bypassed the original CD check".into());
            }
            if local_audio_requested {
                actions.push("enabled Win95-safe BGM.dat streaming".into());
                actions.push("patched the BGM slider for local DirectSound audio".into());
            }
            if modern_volume {
                actions.push("enabled modern SFX volume control".into());
            }
            return Ok(CompatibilityPatch {
                bytes,
                actions,
                local_audio_supported: true,
            });
        }
        if !no_reg {
            return Ok(CompatibilityPatch {
                bytes: input.to_vec(),
                actions: Vec::new(),
                local_audio_supported: false,
            });
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
            return Err("this recognized v1.26 layout already bypasses the CD, but its music functions differ; local BGM.dat playback is not yet safe for this layout".into());
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

/// Builds the optional Vietnamese variant and the selected portable executable.
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
    fn title_credit_preserves_arbitrary_version_text() {
        let input = b"noise Version 2.07-beta\0tail";
        let (raw, length) = find_version_string(input).unwrap();
        assert_eq!(&input[raw..raw + length], b"Version 2.07-beta");
        assert_eq!(
            title_credit(input).unwrap(),
            b"Version 2.07-beta - Patched by Thang\0"
        );
    }

    #[test]
    fn portable_hook_labels_point_to_instruction_boundaries() {
        let (section, labels) = portable_section(true, b"\0").unwrap();
        assert_eq!(
            &section[PORT_BGM_OFFSET..PORT_BGM_OFFSET + BGM_RUNTIME.len()],
            BGM_RUNTIME
        );
        assert!(
            (PORT_VA + PORT_BGM_OFFSET as u32..PORT_VA + PORT_SIZE as u32)
                .contains(&bgm_symbols::BGMDISPATCH)
        );
        let dispatch = (labels["audio_dispatch"] - PORT_VA) as usize;
        assert_eq!(
            u32::from_le_bytes(section[dispatch..dispatch + 4].try_into().unwrap()),
            bgm_symbols::BGMDISPATCH
        );
        assert!(find_bytes(&section, b"BGM.dat\0").is_some());
        assert!(find_bytes(&section, b"BGMRT3\0").is_some());
        assert!(find_bytes(&section, b"doraudio.dll\0").is_none());
        let open = (labels["open_music"] - PORT_VA) as usize;
        let play = (labels["play"] - PORT_VA) as usize;
        assert!(section[open..play]
            .windows(3)
            .any(|instruction| instruction == [0x8b, 0x55, 0x90]));
        assert!(section[open..play]
            .windows(8)
            .any(|instruction| instruction == [0x89, 0xd0, 0x8b, 0xe5, 0x5d, 0xc2, 0x04, 0x00]));
        let stop = (labels["stop"] - PORT_VA) as usize;
        assert!(section[play..stop]
            .windows(9)
            .any(|instruction| instruction == [0x8b, 0x55, 0x08, 0x81, 0xe2, 0xff, 0, 0, 0]));
        for (label, opcode) in [
            ("wave_bgm_get_volume", 0xa1),
            ("wave_bgm_volume", 0x60),
            ("sfx_get_volume", 0x8b),
            ("sfx_slider_volume", 0x60),
            ("sfx_set_volume_1", 0xc7),
            ("sfx_set_volume_2", 0xc7),
        ] {
            let offset = (labels[label] - PORT_VA) as usize;
            assert_eq!(
                section[offset], opcode,
                "{label} is not at its instruction boundary"
            );
        }
        assert!((labels["wave_bgm_volume"] as usize) < PORT_VA as usize + PORT_SFX_OFFSET);
        assert!((labels["sfx_get_volume"] as usize) >= PORT_VA as usize + PORT_SFX_OFFSET);
        let sfx_get = (labels["sfx_get_volume"] - PORT_VA) as usize;
        let sfx_slider = (labels["sfx_slider_volume"] - PORT_VA) as usize;
        assert!(section[sfx_get..sfx_slider]
            .windows(10)
            .any(|instruction| { instruction == [0xc7, 0x81, 0x80, 0, 0, 0, 0xff, 0xff, 0, 0] }));
        let current_level = (labels["sfx_current_level"] - PORT_VA) as usize;
        assert_eq!(
            &section[current_level..current_level + 4],
            &[0xff, 0xff, 0, 0]
        );
    }

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
        assert_ne!(hash::bytes(&portable), hash::bytes(&original));
        assert_eq!(
            &portable[0x8b458..0x8b45d],
            &[0x05, 0xb4, 0x01, 0x00, 0x00],
            "local DirectSound BGM keeps the original slider value calculation"
        );
        assert_eq!(
            portable[0x8b460], 0xe9,
            "local BGM volume must call the DirectSound hook"
        );
        assert_eq!(
            &portable[0x8b402..0x8b404],
            &[0x90, 0x90],
            "local BGM must bypass the absent legacy mixer-control guard"
        );
        assert_eq!(
            &portable[0x8b41b..0x8b422],
            &[0x69, 0xc0, 0xff, 0xff, 0x00, 0x00, 0x90],
            "local BGM must use the full 16-bit DirectSound slider range"
        );
        assert_eq!(
            &portable[0x8a3e5..0x8a3ef],
            &[0xc7, 0x85, 0x50, 0xfe, 0xff, 0xff, 0xd4, 0x00, 0x00, 0x00],
            "local music at full volume must initialize its bell at full width"
        );
        assert_eq!(
            &portable[0x8b3b0..0x8b3b6],
            &[0xff, 0x15, 0x68, 0x92, 0x4b, 0x00],
            "portable patch must preserve the original SFX slider"
        );

        let no_local_audio = patch_compatible(&original, true, false, true, false).unwrap();
        assert_ne!(
            no_local_audio.bytes[0x85043], 0xe9,
            "disabling local music must preserve the original CD-audio routine"
        );
        assert_eq!(
            no_local_audio.bytes[0x8b460], 0xff,
            "disabling local music must preserve the original Music-slider routine"
        );
        assert_eq!(
            &no_local_audio.bytes[0x8b402..0x8b404],
            &[0x74, 0x7c],
            "CD audio must retain the original legacy mixer-control guard"
        );
        assert_eq!(
            &no_local_audio.bytes[0x8b41b..0x8b422],
            &[0x0f, 0xaf, 0x82, 0x20, 0x02, 0x00, 0x00],
            "CD audio must retain its mixer-provided volume range"
        );
        assert_eq!(
            &no_local_audio.bytes[0x8a3eb..0x8a3ef],
            &[0x00, 0x00, 0x00, 0x00],
            "CD audio must retain the original missing-mixer fallback"
        );
        assert_eq!(
            &no_local_audio.bytes[0x8b3b0..0x8b3b6],
            &[0xff, 0x15, 0x68, 0x92, 0x4b, 0x00],
            "disabling local music must preserve the original SFX slider"
        );
        let local_upgrade =
            patch_compatible(&no_local_audio.bytes, true, true, true, false).unwrap();
        assert_eq!(local_upgrade.bytes[0x85043], 0xe9);
        assert_eq!(local_upgrade.bytes[0x851d9], 0xe9);
        assert_eq!(local_upgrade.bytes[0x85366], 0xe9);
        assert_eq!(local_upgrade.bytes[0x8b460], 0xe9);
        assert_eq!(&local_upgrade.bytes[0x8b402..0x8b404], &[0x90, 0x90]);
        assert_eq!(
            &local_upgrade.bytes[0x8b41b..0x8b422],
            &[0x69, 0xc0, 0xff, 0xff, 0x00, 0x00, 0x90]
        );
        assert_eq!(
            &local_upgrade.bytes[0x8a3eb..0x8a3ef],
            &[0xd4, 0x00, 0x00, 0x00]
        );
        let local_disabled =
            patch_compatible(&local_upgrade.bytes, true, false, true, false).unwrap();
        assert_eq!(
            &local_disabled.bytes[0x85043..0x85048],
            &[0x8b, 0x55, 0x08, 0x89, 0x55]
        );
        assert_eq!(
            &local_disabled.bytes[0x851d9..0x851de],
            &[0x55, 0x8b, 0xec, 0x83, 0xec]
        );
        assert_eq!(
            &local_disabled.bytes[0x8b460..0x8b466],
            &[0xff, 0x15, 0x68, 0x92, 0x4b, 0x00]
        );
        assert_eq!(&local_disabled.bytes[0x8b402..0x8b404], &[0x74, 0x7c]);
        assert_eq!(
            &local_disabled.bytes[0x8b41b..0x8b422],
            &[0x0f, 0xaf, 0x82, 0x20, 0x02, 0x00, 0x00]
        );
        assert_eq!(
            &local_disabled.bytes[0x8a3eb..0x8a3ef],
            &[0x00, 0x00, 0x00, 0x00]
        );
        let modern_volume = patch_compatible(&original, true, true, true, true).unwrap();
        assert_eq!(modern_volume.bytes[0x8b3b0], 0xe8);
        assert_eq!(modern_volume.bytes[0x89218], 0xe8);
        assert_eq!(modern_volume.bytes[0x89514], 0xe8);
        assert_eq!(modern_volume.bytes[0x8b460], 0xe9);
        assert_eq!(
            &modern_volume.bytes[0x8b334..0x8b33a],
            &[0x90; 6],
            "modern SFX must bypass the absent legacy mixer-control guard"
        );
        assert_eq!(
            &modern_volume.bytes[0x8b351..0x8b358],
            &[0x69, 0xc0, 0xff, 0xff, 0x00, 0x00, 0x90],
            "modern SFX must use the full 16-bit slider range"
        );
        assert_eq!(
            &modern_volume.bytes[0x8a271..0x8a27b],
            &[0xc7, 0x85, 0x50, 0xfe, 0xff, 0xff, 0xd4, 0x00, 0x00, 0x00],
            "the missing-mixer fallback must place the SFX bell at full width"
        );
        for (source_raw, opcode) in [
            (0x8a162, 0x8b),
            (0x8b3b0, 0x60),
            (0x89218, 0xc7),
            (0x89514, 0xc7),
        ] {
            let displacement = i32::from_le_bytes(
                modern_volume.bytes[source_raw + 1..source_raw + 5]
                    .try_into()
                    .unwrap(),
            );
            let target_va = (IMAGE_BASE + source_raw as u32 + 5).wrapping_add(displacement as u32);
            let port = find_section(&modern_volume.bytes, b".port").unwrap();
            let port_raw = u32::from_le_bytes(
                modern_volume.bytes[port + 20..port + 24]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let target_raw = port_raw + (target_va - PORT_VA) as usize;
            assert_eq!(
                modern_volume.bytes[target_raw],
                opcode,
                "hook at {:#x} does not land on an instruction boundary",
                IMAGE_BASE + source_raw as u32
            );
        }
        let expected_credit = title_credit(&portable).unwrap();
        assert!(find_bytes(&portable, &expected_credit).is_some());
        let (title_raw, _) = find_version_string(&portable).unwrap();
        let title_va = raw_to_va(&portable, title_raw).unwrap();
        assert_eq!(
            portable
                .windows(5)
                .filter(|instruction| instruction[0] == 0x68
                    && instruction[1..] == title_va.to_le_bytes())
                .count(),
            0
        );
        // An EXE patched by the previous portable build did not contain this
        // credit. It must be safely upgradable without restoring first.
        let mut old_portable = portable.clone();
        let credit_raw = find_bytes(&old_portable, &expected_credit).unwrap();
        let credit_va = raw_to_va(&old_portable, credit_raw).unwrap();
        old_portable[credit_raw..credit_raw + expected_credit.len()].fill(0);
        for offset in 1..old_portable.len().saturating_sub(4) {
            if old_portable[offset - 1] == 0x68
                && u32::from_le_bytes(old_portable[offset..offset + 4].try_into().unwrap())
                    == credit_va
            {
                old_portable[offset..offset + 4].copy_from_slice(&title_va.to_le_bytes());
            }
        }
        let upgraded = patch_compatible(&old_portable, true, false, true, false).unwrap();
        assert!(find_bytes(&upgraded.bytes, &expected_credit).is_some());
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
        let portable = patch_compatible(&canonical, true, false, true, false).unwrap();
        assert_eq!(portable.bytes.len(), canonical.len() + PORT_SIZE);
        assert!(portable.local_audio_supported);

        let december = std::fs::read(december_path).unwrap();
        let fixed = patch_compatible(&december, true, false, true, false).unwrap();
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
        let canonical = patch_language_runtime(&canonical, true, true, true, false, false).unwrap();
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
        let migrated = patch_language_runtime(&legacy, true, false, true, false, false).unwrap();
        assert!(has_vietnamese_font_hook(&migrated.bytes));
        assert_eq!(
            &migrated.bytes[layout.cseg_raw + 0x11d0..layout.cseg_raw + 0x11d5],
            &[0x05, 0xbb, 0x3f, 0x00, 0x00]
        );

        let december = std::fs::read(december_path).unwrap();
        let december = patch_language_runtime(&december, true, true, true, false, false).unwrap();
        assert!(has_vietnamese_font_hook(&december.bytes));
        assert!(!december.local_audio_supported);
    }
}

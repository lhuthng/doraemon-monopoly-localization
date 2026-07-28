//! Small 32-bit x86 assembler used by the executable patcher.
//!
//! This module emits the handful of instructions needed for runtime caves.
//! Patch code contains relative branches and absolute data references, so
//! calculating offsets by hand would be error-prone. Labels are recorded as
//! bytes are emitted, then unresolved four-byte fixups are resolved by
//! `finish` once every target address is known.

use std::collections::HashMap;

use crate::Result;

pub(crate) const IMAGE_BASE: u32 = 0x0040_0000;

#[derive(Clone)]
pub(crate) enum Target {
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
pub(crate) struct Asm {
    base: u32,
    pub(crate) bytes: Vec<u8>,
    pub(crate) labels: HashMap<&'static str, u32>,
    fixups: Vec<Fixup>,
}

impl Asm {
    /// Creates an assembler whose first emitted byte has the supplied VA.
    pub(crate) fn new(base: u32) -> Self {
        Self {
            base,
            ..Self::default()
        }
    }

    /// Returns the virtual address immediately after the emitted bytes.
    fn va(&self) -> u32 {
        self.base + self.bytes.len() as u32
    }

    /// Appends literal machine-code bytes.
    pub(crate) fn emit(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    /// Appends a little-endian 32-bit immediate or address.
    pub(crate) fn u32(&mut self, value: u32) {
        self.emit(&value.to_le_bytes());
    }

    /// Names the current location for a later branch or data reference.
    pub(crate) fn label(&mut self, name: &'static str) {
        self.labels.insert(name, self.va());
    }

    /// Emits an opcode followed by a deferred four-byte fixup.
    pub(crate) fn fix(&mut self, opcode: &[u8], target: Target, relative: bool) {
        self.emit(opcode);
        self.fixups.push(Fixup {
            offset: self.bytes.len(),
            target,
            relative,
        });
        self.u32(0);
    }

    /// Emits a near relative jump.
    pub(crate) fn jmp(&mut self, target: Target) {
        self.fix(&[0xe9], target, true);
    }

    /// Emits a near relative call.
    pub(crate) fn call(&mut self, target: Target) {
        self.fix(&[0xe8], target, true);
    }

    /// Emits a near conditional-equal branch.
    pub(crate) fn je(&mut self, target: Target) {
        self.fix(&[0x0f, 0x84], target, true);
    }

    /// Emits a near conditional-not-equal branch.
    pub(crate) fn jne(&mut self, target: Target) {
        self.fix(&[0x0f, 0x85], target, true);
    }

    /// Emits a near unsigned-less-or-equal branch.
    pub(crate) fn jbe(&mut self, target: Target) {
        self.fix(&[0x0f, 0x86], target, true);
    }

    /// Emits a near signed-greater-or-equal branch.
    pub(crate) fn jge(&mut self, target: Target) {
        self.fix(&[0x0f, 0x8d], target, true);
    }

    /// Emits an instruction whose operand is an absolute label address.
    pub(crate) fn absolute(&mut self, opcode: &[u8], label: &'static str) {
        self.fix(opcode, Target::Label(label), false);
    }

    /// Emits an indirect call through an imported function pointer.
    pub(crate) fn call_iat(&mut self, address: u32) {
        self.emit(&[0xff, 0x15]);
        self.u32(address);
    }

    /// Resolves every label and returns the final bytes plus label addresses.
    pub(crate) fn finish(mut self) -> Result<(Vec<u8>, HashMap<&'static str, u32>)> {
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

/// Replaces an existing instruction range with a near jump and NOP padding.
pub(crate) fn patch_jump(output: &mut [u8], va: u32, target: u32, replaced: usize) -> Result<()> {
    let raw = (va - IMAGE_BASE) as usize;
    let bytes = output
        .get_mut(raw..raw + replaced)
        .ok_or("executable jump patch is outside file")?;
    bytes.fill(0x90);
    bytes[0] = 0xe9;
    bytes[1..5].copy_from_slice(&target.wrapping_sub(va + 5).to_le_bytes());
    Ok(())
}

/// Replaces an existing instruction range with a near call and NOP padding.
pub(crate) fn patch_call(output: &mut [u8], va: u32, target: u32, replaced: usize) -> Result<()> {
    let raw = (va - IMAGE_BASE) as usize;
    let bytes = output
        .get_mut(raw..raw + replaced)
        .ok_or("executable call patch is outside file")?;
    bytes.fill(0x90);
    bytes[0] = 0xe8;
    bytes[1..5].copy_from_slice(&target.wrapping_sub(va + 5).to_le_bytes());
    Ok(())
}

/// Patches a five-byte call site in the executable's CSEG section.
pub(crate) fn patch_cseg_jump(
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

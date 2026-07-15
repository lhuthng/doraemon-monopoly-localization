import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';

const EXPECTED_SHA256 = 'fdf00e681671f93b09d257f77d7ce0720e7129cf6bc44ba9e0f19c2efa4fecba';
const CSEG_VA = 0x004d1000;
const CSEG_RAW = 0x000cc000;
const CAVE_VA = 0x004d2c00;
const CAVE_RAW = 0x000cdc00;
const SYSFONT_FILENAME_RAW = 0x000cb00a;
const SYSFONT_FILENAME_CAPACITY = 16;
const VIETNAMESE_SYSFONT_FILENAME = 'sysfont-vi.dat';

type Fixup = { offset: number; target: string | number };

class X86 {
  bytes: number[] = [];
  labels = new Map<string, number>();
  fixups: Fixup[] = [];

  get va() {
    return CAVE_VA + this.bytes.length;
  }
  label(name: string) {
    this.labels.set(name, this.va);
  }
  emit(...bytes: number[]) {
    this.bytes.push(...bytes);
  }
  rel32(opcode: number[], target: string | number) {
    this.emit(...opcode);
    this.fixups.push({ offset: this.bytes.length, target });
    this.emit(0, 0, 0, 0);
  }
  jmp(target: string | number) {
    this.rel32([0xe9], target);
  }
  je(target: string | number) {
    this.rel32([0x0f, 0x84], target);
  }
  jne(target: string | number) {
    this.rel32([0x0f, 0x85], target);
  }
  ja(target: string | number) {
    this.rel32([0x0f, 0x87], target);
  }
  finish() {
    const output = Uint8Array.from(this.bytes);
    for (const fixup of this.fixups) {
      const target = typeof fixup.target === 'number' ? fixup.target : this.labels.get(fixup.target);
      if (target === undefined) throw new Error(`Missing assembly label ${fixup.target}.`);
      const next = CAVE_VA + fixup.offset + 4;
      new DataView(output.buffer).setInt32(fixup.offset, target - next, true);
    }
    return output;
  }
}

function emitVietnameseIndex(asm: X86) {
  // Input: AL=prefix, ESI points at second byte. Output: EAX=absolute glyph index.
  asm.emit(0x0f, 0xb6, 0xc0); // movzx eax,al
  asm.emit(0x2d, 0xcc, 0x00, 0x00, 0x00); // sub eax,0xcc
  asm.emit(0xc1, 0xe0, 0x07); // shl eax,7
  asm.emit(0x0f, 0xb6, 0x0e); // movzx ecx,byte [esi]
  asm.emit(0x01, 0xc8); // add eax,ecx
  asm.emit(0x8b, 0x0d, 0x1a, 0x00, 0x4d, 0x00); // mov ecx,[0x4d001a]
  asm.emit(0x83, 0xf9, 0x04); // cmp ecx,4
  asm.emit(0x76, 0x02); // jbe +2
  asm.emit(0x31, 0xc9); // xor ecx,ecx
  asm.emit(0xc1, 0xe1, 0x08); // shl ecx,8
  asm.emit(0x01, 0xc8); // add eax,ecx
  asm.emit(0x05, 0x80, 0x02, 0x00, 0x00); // add eax,640
}

function emitPrefixCheck(asm: X86, chinese: string) {
  asm.emit(0x3c, 0xcc); // cmp al,0xcc
  asm.jne('check_cd_' + chinese);
  asm.jmp('check_second_' + chinese);
  asm.label('check_cd_' + chinese);
  asm.emit(0x3c, 0xcd); // cmp al,0xcd
  asm.jne(chinese);
  asm.label('check_second_' + chinese);
  asm.emit(0xf6, 0x06, 0x80); // test byte [esi],0x80
  asm.jne(chinese);
}

function buildCave() {
  const a = new X86();

  a.label('measure_string');
  emitPrefixCheck(a, 'measure_chinese');
  emitVietnameseIndex(a);
  a.emit(0x8b, 0x0d, 0x06, 0x00, 0x4d, 0x00); // mov ecx,[sysfont]
  a.emit(0x8b, 0x44, 0x81, 0x02); // mov eax,[ecx+eax*4+2]
  a.emit(0x01, 0xc8); // add eax,ecx
  a.emit(0x0f, 0xb6, 0x00); // movzx eax,byte [eax]
  a.emit(0x01, 0xc2); // add edx,eax
  a.emit(0x46); // inc esi
  a.jmp(0x004d11a8);
  a.label('measure_chinese');
  a.emit(0x46); // inc esi
  a.emit(0x83, 0xc2, 0x10); // add edx,16
  a.jmp(0x004d11a8);

  a.label('character_width');
  a.emit(0x51); // push ecx
  emitPrefixCheck(a, 'width_chinese');
  emitVietnameseIndex(a);
  a.emit(0x8b, 0x0d, 0x06, 0x00, 0x4d, 0x00);
  a.emit(0x8b, 0x44, 0x81, 0x02);
  a.emit(0x01, 0xc8);
  a.emit(0x0f, 0xb6, 0x00);
  a.emit(0x59); // pop ecx
  a.jmp(0x004d123a);
  a.label('width_chinese');
  a.emit(0xb8, 0x10, 0x00, 0x00, 0x00);
  a.emit(0x59);
  a.jmp(0x004d123a);

  a.label('single_render');
  emitPrefixCheck(a, 'single_chinese');
  emitVietnameseIndex(a);
  a.emit(0x46); // consume second byte
  a.emit(0x8b, 0x0d, 0x06, 0x00, 0x4d, 0x00);
  a.emit(0x8b, 0x44, 0x81, 0x02);
  a.emit(0x01, 0xc8);
  a.emit(0x89, 0xc6); // mov esi,eax
  a.jmp(0x004d1281);
  a.label('single_chinese');
  a.emit(0x88, 0xc4); // mov ah,al
  a.emit(0xac); // lodsb
  a.emit(0x66, 0x25, 0xff, 0x7f); // and ax,0x7fff
  a.jmp(0x004d12e8);

  a.label('string_render');
  emitPrefixCheck(a, 'string_chinese');
  emitVietnameseIndex(a);
  a.emit(0x46); // consume second byte
  a.emit(0x56); // save updated source pointer
  a.emit(0x8b, 0x0d, 0x06, 0x00, 0x4d, 0x00);
  a.emit(0x8b, 0x44, 0x81, 0x02);
  a.emit(0x01, 0xc8);
  a.emit(0x89, 0xc6);
  a.jmp(0x004d140b);
  a.label('string_chinese');
  a.emit(0x88, 0xc4); // mov ah,al
  a.emit(0xac); // lodsb
  a.emit(0x56); // push esi
  a.emit(0x66, 0x25, 0xff, 0x7f);
  a.jmp(0x004d144c);

  return { bytes: a.finish(), labels: a.labels };
}

function vaToRaw(va: number) {
  return CSEG_RAW + (va - CSEG_VA);
}

function patchJump(output: Uint8Array, source: number, target: number) {
  const raw = vaToRaw(source);
  output[raw] = 0xe9;
  new DataView(output.buffer).setInt32(raw + 1, target - (source + 5), true);
}

function findCsegSection(output: Uint8Array) {
  const view = new DataView(output.buffer, output.byteOffset, output.byteLength);
  const pe = view.getUint32(0x3c, true);
  const count = view.getUint16(pe + 6, true);
  const optionalSize = view.getUint16(pe + 20, true);
  const sections = pe + 24 + optionalSize;
  for (let index = 0; index < count; index += 1) {
    const offset = sections + index * 40;
    const name = new TextDecoder().decode(output.slice(offset, offset + 8)).replaceAll('\0', '');
    if (name === 'CSEG') return offset;
  }
  throw new Error('Doraemon.exe has no CSEG section.');
}

export function patchVietnameseExecutable(original: Uint8Array) {
  const digest = createHash('sha256').update(original).digest('hex');
  if (digest !== EXPECTED_SHA256) {
    throw new Error(`Unsupported Doraemon.exe (SHA-256 ${digest}). Expected ${EXPECTED_SHA256}.`);
  }
  const output = new Uint8Array(original);
  const fontFilename = new TextEncoder().encode(`${VIETNAMESE_SYSFONT_FILENAME}\0`);
  if (fontFilename.length > SYSFONT_FILENAME_CAPACITY) {
    throw new Error(`${VIETNAMESE_SYSFONT_FILENAME} does not fit the executable filename buffer.`);
  }
  output.fill(0, SYSFONT_FILENAME_RAW, SYSFONT_FILENAME_RAW + SYSFONT_FILENAME_CAPACITY);
  output.set(fontFilename, SYSFONT_FILENAME_RAW);
  const cave = buildCave();
  if (cave.bytes.length > 0x400)
    throw new Error(`Patch requires ${cave.bytes.length} cave bytes; only 1024 exist.`);
  output.fill(0x90, CAVE_RAW, CAVE_RAW + 0x400);
  output.set(cave.bytes, CAVE_RAW);
  patchJump(output, 0x004d11d0, cave.labels.get('measure_string')!);
  patchJump(output, 0x004d1235, cave.labels.get('character_width')!);
  patchJump(output, 0x004d12e1, cave.labels.get('single_render')!);
  patchJump(output, 0x004d1444, cave.labels.get('string_render')!);

  const section = findCsegSection(output);
  const view = new DataView(output.buffer);
  view.setUint32(section + 8, 0x2000, true); // VirtualSize includes the patch cave.
  view.setUint32(section + 36, view.getUint32(section + 36, true) | 0x20000000, true); // executable
  return { output, caveBytes: cave.bytes.length };
}

if (import.meta.main) {
  const input = process.argv[2];
  const output = process.argv[3];
  if (!input || !output) throw new Error('Usage: bun patch-vietnamese-exe.ts INPUT.EXE OUTPUT.EXE');
  const patched = patchVietnameseExecutable(new Uint8Array(readFileSync(input)));
  writeFileSync(output, patched.output);
  console.log(`Patched ${output}; used ${patched.caveBytes}/1024 bytes of CSEG cave.`);
}

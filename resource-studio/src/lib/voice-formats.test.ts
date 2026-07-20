import { describe, expect, test } from 'bun:test';
import { compressGameOneRecord } from './formats';
import {
  decodeVoiceRecord,
  encodePcmWav,
  parseVoiceArchive,
  parseWav,
  rebuildVoiceArchive,
  VOICE_SAMPLE_RATE
} from './voice-formats';

const signature = new TextEncoder().encode('\0\0GameOne Systems Limited\nWritten by Samme NG\0');

function putU32(bytes: Uint8Array, offset: number, value: number) {
  new DataView(bytes.buffer).setUint32(offset, value, true);
}

function container(children: Uint8Array[]) {
  const headerSize = 0x66 + (children.length + 1) * 4;
  const output = new Uint8Array(headerSize + children.reduce((sum, child) => sum + child.length, 0));
  output.set(signature);
  putU32(output, 0x42, children.length);
  let cursor = headerSize;
  children.forEach((child, index) => {
    putU32(output, 0x66 + index * 4, cursor);
    output.set(child, cursor);
    cursor += child.length;
  });
  putU32(output, 0x66 + children.length * 4, cursor);
  return output;
}

function voiceFixture(bankSizes: number[]) {
  const wav = encodePcmWav(Float32Array.from([0, 0.25, -0.25, 0.5]));
  return container(
    Array.from({ length: 6 }, () =>
      container(
        bankSizes.map((size, bank) =>
          container(
            Array.from({ length: size }, (_, slot) => {
              if (bank === 0 && slot === 0) return wav;
              if (bank === 0 && slot === 1) return compressGameOneRecord(wav);
              return Uint8Array.of(0x23);
            })
          )
        )
      )
    )
  );
}

describe('Voice.dat', () => {
  test.each([
    [[84, 64, 42], 1_140],
    [[89, 64, 42, 37], 1_392]
  ])('recognizes the known %p topology', (banks, total) => {
    const archive = parseVoiceArchive(voiceFixture(banks));
    expect(archive.characters).toBe(6);
    expect(archive.bankCounts).toEqual(Array.from({ length: 6 }, () => banks));
    expect(archive.records).toHaveLength(total);
  });

  test('decodes raw, compressed, and empty records lazily', () => {
    const archive = parseVoiceArchive(voiceFixture([3]));
    expect(archive.records.slice(0, 3).map((record) => record.storage)).toEqual([
      'raw',
      'compressed',
      'empty'
    ]);
    expect(decodeVoiceRecord(archive, archive.records[0])).toEqual(
      decodeVoiceRecord(archive, archive.records[1])
    );
    expect(decodeVoiceRecord(archive, archive.records[2])).toBeUndefined();
  });

  test('rebuilds changed records while an unchanged export stays byte-identical', () => {
    const original = voiceFixture([4]);
    const archive = parseVoiceArchive(original);
    expect(rebuildVoiceArchive(archive, new Map())).toEqual(original);

    const replacement = encodePcmWav(Float32Array.from([1, -1, 0.5, -0.5, 0]));
    const rebuilt = rebuildVoiceArchive(
      archive,
      new Map([
        ['000/000/000', replacement],
        ['000/000/001', replacement],
        ['000/000/002', replacement]
      ])
    );
    const verified = parseVoiceArchive(rebuilt);
    for (const id of ['000/000/000', '000/000/001', '000/000/002']) {
      const record = verified.records.find((candidate) => candidate.id === id)!;
      expect(decodeVoiceRecord(verified, record)).toEqual(replacement);
    }
    expect(verified.records.find((record) => record.id === '000/000/003')?.storage).toBe('empty');
  });

  test('writes the canonical game PCM WAV format', () => {
    const wav = encodePcmWav(new Float32Array(VOICE_SAMPLE_RATE));
    expect(parseWav(wav)).toMatchObject({
      format: 1,
      channels: 1,
      sampleRate: VOICE_SAMPLE_RATE,
      bitsPerSample: 16,
      duration: 1
    });
  });
});

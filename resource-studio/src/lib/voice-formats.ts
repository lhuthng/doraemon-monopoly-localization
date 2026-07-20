import {
  compressGameOneRecord,
  decompressGameOneRecord,
  listGameOneArchive,
  rebuildGameOneArchivePacked
} from './formats';

export const VOICE_CACHE_BYTES = 0x64000;
export const VOICE_SAMPLE_RATE = 22_050;

export type VoiceStorage = 'raw' | 'compressed' | 'empty';

export type VoiceRecord = {
  id: string;
  path: [number, number, number];
  offset: number;
  end: number;
  storage: VoiceStorage;
};

export type VoiceArchive = {
  bytes: Uint8Array;
  records: VoiceRecord[];
  characters: number;
  bankCounts: number[][];
};

export type WavInfo = {
  format: number;
  channels: number;
  sampleRate: number;
  bitsPerSample: number;
  dataOffset: number;
  dataLength: number;
  duration: number;
};

const RIFF = new TextEncoder().encode('RIFF');
const WAVE = new TextEncoder().encode('WAVE');

function matches(bytes: Uint8Array, offset: number, expected: Uint8Array) {
  return expected.every((byte, index) => bytes[offset + index] === byte);
}

function u16(bytes: Uint8Array, offset: number) {
  return bytes[offset] | (bytes[offset + 1] << 8);
}

function u32(bytes: Uint8Array, offset: number) {
  return (
    (bytes[offset] | (bytes[offset + 1] << 8) | (bytes[offset + 2] << 16) | (bytes[offset + 3] << 24)) >>> 0
  );
}

function putU16(bytes: Uint8Array, offset: number, value: number) {
  bytes[offset] = value & 0xff;
  bytes[offset + 1] = (value >>> 8) & 0xff;
}

function putU32(bytes: Uint8Array, offset: number, value: number) {
  bytes[offset] = value & 0xff;
  bytes[offset + 1] = (value >>> 8) & 0xff;
  bytes[offset + 2] = (value >>> 16) & 0xff;
  bytes[offset + 3] = (value >>> 24) & 0xff;
}

export function parseWav(bytes: Uint8Array): WavInfo {
  if (bytes.length < 44 || !matches(bytes, 0, RIFF) || !matches(bytes, 8, WAVE)) {
    throw new Error('Audio record is not a RIFF/WAVE file.');
  }
  let cursor = 12;
  let format: { format: number; channels: number; sampleRate: number; bitsPerSample: number } | undefined;
  let dataOffset = -1;
  let dataLength = 0;
  while (cursor + 8 <= bytes.length) {
    const name = new TextDecoder('latin1').decode(bytes.subarray(cursor, cursor + 4));
    const length = u32(bytes, cursor + 4);
    const start = cursor + 8;
    const end = start + length;
    if (end > bytes.length) throw new Error(`WAV chunk ${name} extends beyond the record.`);
    if (name === 'fmt ') {
      if (length < 16) throw new Error('WAV fmt chunk is too small.');
      format = {
        format: u16(bytes, start),
        channels: u16(bytes, start + 2),
        sampleRate: u32(bytes, start + 4),
        bitsPerSample: u16(bytes, start + 14)
      };
    } else if (name === 'data') {
      dataOffset = start;
      dataLength = length;
    }
    cursor = end + (length & 1);
  }
  if (!format || dataOffset < 0) throw new Error('WAV is missing its fmt or data chunk.');
  const bytesPerFrame = format.channels * Math.ceil(format.bitsPerSample / 8);
  if (!bytesPerFrame || !format.sampleRate) throw new Error('WAV declares an invalid sample format.');
  return {
    ...format,
    dataOffset,
    dataLength,
    duration: dataLength / bytesPerFrame / format.sampleRate
  };
}

export function parseVoiceArchive(bytes: Uint8Array): VoiceArchive {
  const leaves = listGameOneArchive(bytes).filter((node) => !node.container);
  const records: VoiceRecord[] = leaves.map((node) => {
    if (node.path.length !== 3)
      throw new Error(`Voice record ${node.path.join('/')} does not have character/bank/slot coordinates.`);
    const packed = bytes.subarray(node.offset, node.end);
    const storage: VoiceStorage =
      packed.length === 1 ? 'empty' : matches(packed, 0, RIFF) ? 'raw' : 'compressed';
    const path = node.path as [number, number, number];
    return {
      id: path.map((part) => String(part).padStart(3, '0')).join('/'),
      path,
      offset: node.offset,
      end: node.end,
      storage
    };
  });
  const characters = Math.max(-1, ...records.map((record) => record.path[0])) + 1;
  const bankCounts = Array.from({ length: characters }, (_, character) => {
    const bankTotal =
      Math.max(
        -1,
        ...records.filter((record) => record.path[0] === character).map((record) => record.path[1])
      ) + 1;
    return Array.from(
      { length: bankTotal },
      (_, bank) => records.filter((record) => record.path[0] === character && record.path[1] === bank).length
    );
  });
  if (characters !== 6) throw new Error(`Voice archive has ${characters} characters instead of 6.`);
  return { bytes, records, characters, bankCounts };
}

export function decodeVoiceRecord(archive: VoiceArchive, record: VoiceRecord) {
  const packed = archive.bytes.subarray(record.offset, record.end);
  if (record.storage === 'empty') return undefined;
  const wav = record.storage === 'raw' ? packed.slice() : decompressGameOneRecord(packed);
  parseWav(wav);
  return wav;
}

export function voiceBankStorage(archive: VoiceArchive, character: number, bank: number) {
  const records = archive.records.filter(
    (record) => record.path[0] === character && record.path[1] === bank && record.storage !== 'empty'
  );
  const raw = records.filter((record) => record.storage === 'raw').length;
  return raw > records.length / 2 ? 'raw' : 'compressed';
}

export function packVoiceReplacement(archive: VoiceArchive, record: VoiceRecord, wav: Uint8Array) {
  const info = parseWav(wav);
  if (
    info.format !== 1 ||
    info.channels !== 1 ||
    info.sampleRate !== VOICE_SAMPLE_RATE ||
    info.bitsPerSample !== 16
  ) {
    throw new Error('Replacement must be normalized to mono 22.05 kHz 16-bit PCM WAV.');
  }
  if (wav.length > VOICE_CACHE_BYTES) {
    throw new Error(
      `Replacement uses ${wav.length.toLocaleString()} decoded bytes; the game cache allows ${VOICE_CACHE_BYTES.toLocaleString()}.`
    );
  }
  const storage =
    record.storage === 'empty' ? voiceBankStorage(archive, record.path[0], record.path[1]) : record.storage;
  return storage === 'raw' ? wav : compressGameOneRecord(wav);
}

export function rebuildVoiceArchive(
  archive: VoiceArchive,
  replacements: ReadonlyMap<string, Uint8Array | null>
) {
  const packed = new Map<string, Uint8Array>();
  const emptyMarker = archive.records.find((record) => record.storage === 'empty');
  const emptyBytes = emptyMarker
    ? archive.bytes.slice(emptyMarker.offset, emptyMarker.end)
    : Uint8Array.of(0x23);
  for (const [id, wav] of replacements) {
    const record = archive.records.find((candidate) => candidate.id === id);
    if (!record) throw new Error(`Voice archive has no record ${id}.`);
    packed.set(id, wav ? packVoiceReplacement(archive, record, wav) : emptyBytes);
  }
  const rebuilt = rebuildGameOneArchivePacked(archive.bytes, packed);
  const verified = parseVoiceArchive(rebuilt);
  if (verified.records.length !== archive.records.length)
    throw new Error('Rebuilt Voice.dat changed the number of records.');
  for (const [id, expected] of replacements) {
    const record = verified.records.find((candidate) => candidate.id === id);
    if (!record) throw new Error(`Rebuilt Voice.dat lost record ${id}.`);
    const actual = decodeVoiceRecord(verified, record);
    if (expected === null) {
      if (actual === undefined) continue;
      throw new Error(`Voice verification failed for cleared record ${id}.`);
    }
    if (
      !actual ||
      actual.length !== expected.length ||
      !actual.every((byte, index) => byte === expected[index])
    ) {
      throw new Error(`Voice verification failed for ${id}.`);
    }
  }
  return rebuilt;
}

export function assertCompatibleVoiceArchives(original: VoiceArchive, modified: VoiceArchive) {
  const originalIds = original.records.map((record) => record.id);
  const modifiedIds = modified.records.map((record) => record.id);
  if (
    originalIds.length !== modifiedIds.length ||
    originalIds.some((id, index) => id !== modifiedIds[index])
  ) {
    throw new Error('Modified voice.dat does not have the same character/bank/slot layout as the original.');
  }
}

export function encodePcmWav(samples: Float32Array, sampleRate = VOICE_SAMPLE_RATE) {
  const output = new Uint8Array(44 + samples.length * 2);
  output.set(RIFF, 0);
  putU32(output, 4, output.length - 8);
  output.set(WAVE, 8);
  output.set(new TextEncoder().encode('fmt '), 12);
  putU32(output, 16, 16);
  putU16(output, 20, 1);
  putU16(output, 22, 1);
  putU32(output, 24, sampleRate);
  putU32(output, 28, sampleRate * 2);
  putU16(output, 32, 2);
  putU16(output, 34, 16);
  output.set(new TextEncoder().encode('data'), 36);
  putU32(output, 40, samples.length * 2);
  const view = new DataView(output.buffer);
  for (let index = 0; index < samples.length; index += 1) {
    const sample = Math.max(-1, Math.min(1, samples[index]));
    view.setInt16(44 + index * 2, sample < 0 ? sample * 0x8000 : sample * 0x7fff, true);
  }
  return output;
}

export async function normalizeAudioFile(file: Blob) {
  const Context = window.AudioContext ?? window.webkitAudioContext;
  if (!Context) throw new Error('This browser cannot decode audio files.');
  const context = new Context();
  try {
    const decoded = await context.decodeAudioData((await file.arrayBuffer()).slice(0));
    const outputLength = Math.ceil((decoded.length * VOICE_SAMPLE_RATE) / decoded.sampleRate);
    const mono = new Float32Array(outputLength);
    for (let output = 0; output < outputLength; output += 1) {
      const source = (output * decoded.sampleRate) / VOICE_SAMPLE_RATE;
      const left = Math.min(decoded.length - 1, Math.floor(source));
      const right = Math.min(decoded.length - 1, left + 1);
      const fraction = source - left;
      let sample = 0;
      for (let channel = 0; channel < decoded.numberOfChannels; channel += 1) {
        const data = decoded.getChannelData(channel);
        sample += data[left] + (data[right] - data[left]) * fraction;
      }
      mono[output] = sample / decoded.numberOfChannels;
    }
    const wav = encodePcmWav(mono);
    if (wav.length > VOICE_CACHE_BYTES) {
      throw new Error(
        `Audio is ${(mono.length / VOICE_SAMPLE_RATE).toFixed(2)} seconds after conversion; the safe limit is ${((VOICE_CACHE_BYTES - 44) / (VOICE_SAMPLE_RATE * 2)).toFixed(2)} seconds.`
      );
    }
    return wav;
  } catch (error) {
    if (
      error instanceof Error &&
      (error.message.includes('safe limit') || error.message.includes('Audio is '))
    )
      throw error;
    throw new Error(
      `The browser could not decode this audio file: ${error instanceof Error ? error.message : String(error)}`,
      { cause: error }
    );
  } finally {
    await context.close();
  }
}

declare global {
  interface Window {
    webkitAudioContext?: typeof AudioContext;
  }
}

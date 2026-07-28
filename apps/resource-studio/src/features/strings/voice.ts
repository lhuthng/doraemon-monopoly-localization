import { parseVoiceArchive, type VoiceArchive } from '@doraemon-monopoly/dubbing-core';

export { dialogueVoicePath, globalActionVoiceSlot } from '@doraemon-monopoly/dubbing-core';

export type PreparedVoiceRecord = {
  id: string;
  path: [number, number, number];
  storage: 'raw' | 'compressed' | 'empty';
  url?: string;
  duration?: number;
  sampleRate?: number;
  bitsPerSample?: number;
  hash?: string;
};

export type PreparedVoiceSource = {
  name: string;
  characters: number;
  bankCounts: number[][];
  records: PreparedVoiceRecord[];
};

export type VoiceManifest = {
  version: number;
  characters: string[];
  original: PreparedVoiceSource;
  working: PreparedVoiceSource;
};

function sameBytes(left: Uint8Array, right: Uint8Array) {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

export function manifestSource(archive: VoiceArchive, name: string, hashes: string[]): PreparedVoiceSource {
  return {
    name,
    characters: archive.characters,
    bankCounts: archive.bankCounts,
    records: archive.records.map((record, index) => ({
      id: record.id,
      path: record.path,
      storage: record.storage,
      hash: hashes[index]
    }))
  };
}

export async function manifestFromArchives(
  originalBytes: Uint8Array,
  originalName: string,
  workingBytes = originalBytes,
  workingName = originalName
): Promise<VoiceManifest> {
  const originalArchive = parseVoiceArchive(originalBytes);
  const workingArchive = parseVoiceArchive(workingBytes);
  const originalHashes: string[] = [];
  const workingHashes: string[] = [];
  for (let index = 0; index < originalArchive.records.length; index += 1) {
    const original = originalArchive.records[index];
    const working = workingArchive.records[index];
    const same =
      working?.id === original.id &&
      sameBytes(
        originalArchive.bytes.subarray(original.offset, original.end),
        workingArchive.bytes.subarray(working.offset, working.end)
      );
    originalHashes.push(same ? `same:${index}` : `original:${index}`);
    workingHashes.push(same ? `same:${index}` : `working:${index}`);
  }
  return {
    version: 1,
    characters: ['Doraemon', 'Nobita', 'Dorami', 'Shizuka', 'Suneo', 'Gian'],
    original: manifestSource(originalArchive, originalName, originalHashes),
    working: manifestSource(workingArchive, workingName, workingHashes)
  };
}

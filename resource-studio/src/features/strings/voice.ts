import { parseVoiceArchive, type VoiceArchive } from '../../lib/voice-formats';

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

/**
 * Resolves a character string record to its physical Voice.dat coordinate.
 *
 * Version 1.26 Cantonese keeps its directly linked dialogue in bank 0. Version
 * 1.18 Chinese continues after bank-0 slot 88 in a sparse 37-record bank 3.
 * The holes are dialogue records for which the archive contains no voice.
 */
export function dialogueVoicePath(
  stringGroup: number,
  stringSlot: number,
  bankCounts: number[][]
): [number, number, number] | undefined {
  if (stringGroup < 3 || stringGroup > 8) return undefined;
  const character = stringGroup - 3;
  const characterBanks = bankCounts[character];
  if (!characterBanks) return undefined;
  if (stringSlot < (characterBanks[0] ?? 0)) return [character, 0, stringSlot];
  if (characterBanks[3] !== 37) return undefined;

  let bankSlot: number | undefined;
  if (stringSlot >= 89 && stringSlot <= 100) bankSlot = stringSlot - 89;
  else if (stringSlot === 102) bankSlot = 12;
  else if (stringSlot >= 104 && stringSlot <= 124) bankSlot = stringSlot - 91;
  else if (stringSlot === 127) bankSlot = 34;
  else if (stringSlot === 128) bankSlot = 35;
  else if (stringSlot === 130) bankSlot = 36;

  return bankSlot === undefined ? undefined : [character, 3, bankSlot];
}

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

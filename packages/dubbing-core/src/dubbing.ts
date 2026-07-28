export const DUBBING_FORMAT = 'doraemon-monopoly-dubbing/v1';

/** Global action text 000/031–035 selects bank-1 slots 011–015 for every character. */
export function globalActionVoiceSlot(stringGroup: number, stringSlot: number) {
  if (stringGroup !== 0 || stringSlot < 31 || stringSlot > 35) return undefined;
  return stringSlot - 20;
}

/**
 * Resolves a character string record to its physical Voice.dat coordinate.
 *
 * Version 1.26 Cantonese keeps its directly linked dialogue in bank 0. Version
 * 1.18 Chinese continues after bank-0 slot 88 in a sparse 37-record bank 3.
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

export const DUBBING_OWNERS = ['doraemon', 'nobita', 'dorami', 'shizuka', 'suneo', 'gian', 'others'] as const;
export const GADGET_RECORD_GROUP = 1;
export const GADGET_RECORD_COUNT = 42;

export type DubbingOwner = (typeof DUBBING_OWNERS)[number];
export type DubbingLanguage = 'english' | 'vietnamese';

export type DubbingRecord = { id: string; translation: string; note?: string };
export type DialogueFile = {
  format: typeof DUBBING_FORMAT;
  owner: DubbingOwner;
  records: DubbingRecord[];
};

export type DubbingManifest = {
  format: typeof DUBBING_FORMAT;
  language: DubbingLanguage;
  stringsSha256?: string | string[];
  voiceSha256?: string | string[];
};

export type ContributionRecord = {
  id: string;
  owner: DubbingOwner;
  source?: string;
  translation?: string;
  voiceId?: string;
  gadgetAssetId?: number;
  gadgetVoiceSlot?: number;
};

export type ContributionCatalogue = {
  format: typeof DUBBING_FORMAT;
  generatedAt: string;
  languages: Record<
    DubbingLanguage,
    { records: ContributionRecord[]; voiceIds: string[]; dubbedVoiceIds: string[] }
  >;
};

const stringId = /^(\d{3})\/(\d{3})$/;
const voiceId = /^(\d{3})\/(\d{3})\/(\d{3})$/;

export function isDubbingLanguage(value: string): value is DubbingLanguage {
  return value === 'english' || value === 'vietnamese';
}

export function isDubbingOwner(value: string): value is DubbingOwner {
  return (DUBBING_OWNERS as readonly string[]).includes(value);
}

export function ownerForStringId(id: string): DubbingOwner | undefined {
  const match = stringId.exec(id);
  if (!match) return undefined;
  const group = Number(match[1]);
  return group >= 3 && group <= 8 ? DUBBING_OWNERS[group - 3] : 'others';
}

export function ownerForVoiceId(id: string): DubbingOwner | undefined {
  const match = voiceId.exec(id);
  if (!match) return undefined;
  const character = Number(match[1]);
  return character >= 0 && character < 6 ? DUBBING_OWNERS[character] : undefined;
}

export function gadgetMetadataForStringId(id: string) {
  const match = stringId.exec(id);
  if (!match || Number(match[1]) !== GADGET_RECORD_GROUP) return undefined;
  const slot = Number(match[2]);
  if (slot < 0 || slot >= GADGET_RECORD_COUNT) return undefined;
  return { assetId: slot, voiceSlot: slot };
}

export function gadgetVoiceId(character: number, slot: number) {
  if (!Number.isInteger(character) || character < 0 || character >= 6)
    throw new Error(`Invalid gadget character slot ${character}.`);
  if (!Number.isInteger(slot) || slot < 0 || slot >= GADGET_RECORD_COUNT)
    throw new Error(`Invalid gadget voice slot ${slot}.`);
  return `${String(character).padStart(3, '0')}/002/${String(slot).padStart(3, '0')}`;
}

export function compareRecordIds(left: string, right: string) {
  const leftParts = left.split('/').map(Number);
  const rightParts = right.split('/').map(Number);
  for (let index = 0; index < Math.max(leftParts.length, rightParts.length); index += 1) {
    const difference = (leftParts[index] ?? -1) - (rightParts[index] ?? -1);
    if (difference) return difference;
  }
  return 0;
}

export function voiceFilename(id: string) {
  if (!voiceId.test(id)) throw new Error(`Invalid voice ID ${JSON.stringify(id)}.`);
  return `${id.replaceAll('/', '-')}.wav`;
}

export function voiceIdFromFilename(name: string) {
  const match = /^(\d{3})-(\d{3})-(\d{3})\.wav$/i.exec(name);
  return match ? `${match[1]}/${match[2]}/${match[3]}` : undefined;
}

export function normalizeDialogueFile(owner: DubbingOwner, input: Partial<DialogueFile>): DialogueFile {
  const seen = new Set<string>();
  const records = (input.records ?? [])
    .map((record) => ({
      id: record.id,
      translation: record.translation.replaceAll('\r\n', '\n').replaceAll('\r', '\n'),
      ...(record.note?.trim() ? { note: record.note.trim() } : {})
    }))
    .filter((record) => record.translation.trim())
    .sort((left, right) => compareRecordIds(left.id, right.id));
  for (const record of records) {
    if (!stringId.test(record.id))
      throw new Error(`Invalid dialogue record ID ${JSON.stringify(record.id)}.`);
    if (ownerForStringId(record.id) !== owner)
      throw new Error(`${record.id} does not belong in dialogue/${owner}.json.`);
    if (seen.has(record.id)) throw new Error(`Duplicate dialogue record ${record.id}.`);
    seen.add(record.id);
  }
  return { format: DUBBING_FORMAT, owner, records };
}

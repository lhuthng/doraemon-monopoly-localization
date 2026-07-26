export const DUBBING_FORMAT = 'doraemon-monopoly-dubbing/v1';

export const DUBBING_OWNERS = ['doraemon', 'nobita', 'dorami', 'shizuka', 'suneo', 'gian', 'others'] as const;

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
  maxLines: number;
  maxWidth?: number;
  voiceId?: string;
};

export type ContributionCatalogue = {
  format: typeof DUBBING_FORMAT;
  generatedAt: string;
  languages: Record<DubbingLanguage, { records: ContributionRecord[] }>;
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

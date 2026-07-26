import { mkdir, readdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  DUBBING_FORMAT,
  DUBBING_OWNERS,
  compareRecordIds,
  gadgetMetadataForStringId,
  isDubbingLanguage,
  normalizeDialogueFile,
  ownerForStringId,
  ownerForVoiceId,
  type DialogueFile,
  type DubbingLanguage,
  type DubbingManifest
} from '../src/lib/dubbing';
import { parseStrings, rebuildStrings, type StringRecord } from '../src/lib/formats';
import { CHIFONT_MAP } from '../src/features/strings/chifont-map';
import {
  decodeVoiceRecord,
  parseVoiceArchive,
  parseWav,
  rebuildVoiceArchive,
  VOICE_SAMPLE_RATE
} from '../src/lib/voice-formats';
import { dialogueVoicePath } from '../src/features/strings/voice';

const studio = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repository = resolve(studio, '..');
const dubbingRoot = resolve(repository, 'dubbing');
const localGame = resolve(studio, 'local-game');
const generatedCatalogue = resolve(studio, 'src/lib/generated-dubbing-catalogue.ts');
const SUPPORTED_STRINGS_HASHES = [
  '9ecce72afcef20e472d70d5d3b642887202ea85fc05e2e22aa05694de972dcec',
  '2cab5b50a88b6ddaba1a2829d555ad5c0d296b9a6d7ad20af7604745341ad38e'
];
const SUPPORTED_VOICE_HASHES = [
  'e1493bad6c543fc5888f4524c166df55fa8a095c9390d646b60e314fd8c89a85',
  '4cf31414b732d523432c36d410523e5cac4fde2b3e7ad5f367e4d7216a00e9e2'
];

function usage(): never {
  throw new Error('Usage: bun scripts/dubbing.ts export|sync|organize|check|catalogue [english|vietnamese]');
}

function sourceText(record: StringRecord) {
  return record.tokens
    .map((token) => {
      if (token.type === 'glyph') return CHIFONT_MAP[token.id] || `⟦g${token.id}⟧`;
      if (token.type === 'newline') return '\n';
      if (token.type === 'end') return '';
      return token.text;
    })
    .join('');
}

async function exists(path: string) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

async function sha256(bytes: Uint8Array) {
  const digest = await crypto.subtle.digest('SHA-256', bytes.slice().buffer);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function languagePath(language: DubbingLanguage) {
  return resolve(dubbingRoot, language);
}

function ownerDialoguePath(language: DubbingLanguage, owner: (typeof DUBBING_OWNERS)[number]) {
  return resolve(languagePath(language), 'dialogue', `${owner}.json`);
}

function workspacePath(language: DubbingLanguage, name: string) {
  return resolve(localGame, language, name);
}

function originPath(name: string) {
  return resolve(localGame, 'origin', name);
}

async function readDialogue(language: DubbingLanguage) {
  const all = new Map<string, string>();
  for (const owner of DUBBING_OWNERS) {
    const path = ownerDialoguePath(language, owner);
    if (!(await exists(path))) continue;
    const parsed = JSON.parse(await readFile(path, 'utf8')) as Partial<DialogueFile>;
    const normalized = normalizeDialogueFile(owner, parsed);
    for (const record of normalized.records) {
      if (all.has(record.id)) throw new Error(`Dialogue record ${record.id} appears more than once.`);
      all.set(record.id, record.translation);
    }
  }
  return all;
}

async function writeDialogue(language: DubbingLanguage, translations: ReadonlyMap<string, string>) {
  for (const owner of DUBBING_OWNERS) {
    const records = [...translations]
      .filter(([id]) => ownerForStringId(id) === owner)
      .map(([id, translation]) => ({ id, translation }));
    const normalized = normalizeDialogueFile(owner, { records });
    const path = ownerDialoguePath(language, owner);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, `${JSON.stringify(normalized, null, 2)}\n`);
  }
}

async function writeManifest(language: DubbingLanguage, strings: Uint8Array, voice: Uint8Array) {
  const stringsHash = await sha256(strings);
  const voiceHash = await sha256(voice);
  const manifest: DubbingManifest = {
    format: DUBBING_FORMAT,
    language,
    stringsSha256: SUPPORTED_STRINGS_HASHES.includes(stringsHash) ? SUPPORTED_STRINGS_HASHES : [stringsHash],
    voiceSha256: SUPPORTED_VOICE_HASHES.includes(voiceHash) ? SUPPORTED_VOICE_HASHES : [voiceHash]
  };
  await mkdir(languagePath(language), { recursive: true });
  await writeFile(resolve(languagePath(language), 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
}

async function readManifest(language: DubbingLanguage) {
  const path = resolve(languagePath(language), 'manifest.json');
  if (!(await exists(path)))
    throw new Error(`Missing ${path}. Run dubbing:export to create the source tree.`);
  const manifest = JSON.parse(await readFile(path, 'utf8')) as DubbingManifest;
  if (manifest.format !== DUBBING_FORMAT || manifest.language !== language)
    throw new Error(`${path} has an incompatible manifest.`);
  return manifest;
}

async function voiceFiles(language: DubbingLanguage) {
  const replacements = new Map<string, Uint8Array>();
  const root = resolve(languagePath(language), 'voices');
  if (!(await exists(root))) return replacements;
  for (const owner of DUBBING_OWNERS) {
    const directory = resolve(root, owner);
    if (!(await exists(directory))) continue;
    for (const entry of await readdir(directory)) {
      if (!entry.toLowerCase().endsWith('.wav')) throw new Error(`Unexpected voice file ${owner}/${entry}.`);
      const path = resolve(directory, entry);
      const bytes = new Uint8Array(await readFile(path));
      const info = parseWav(bytes);
      if (
        info.format !== 1 ||
        info.channels !== 1 ||
        info.sampleRate !== VOICE_SAMPLE_RATE ||
        info.bitsPerSample !== 16
      )
        throw new Error(`${owner}/${entry} must be mono 22.05 kHz 16-bit PCM WAV.`);
      const shared = /^shared-action-(\d{3})\.wav$/i.exec(entry);
      if (shared) {
        if (owner !== 'others' || Number(shared[1]) < 11 || Number(shared[1]) > 15)
          throw new Error(`${owner}/${entry} is not a valid shared action voice.`);
        for (let character = 0; character < 6; character += 1)
          replacements.set(`${String(character).padStart(3, '0')}/001/${shared[1]}`, bytes);
        continue;
      }
      const id = /^(\d{3})-(\d{3})-(\d{3})\.wav$/i.exec(entry);
      if (!id) throw new Error(`Voice filename ${owner}/${entry} must use CCC-BBB-SSS.wav.`);
      const recordId = `${id[1]}/${id[2]}/${id[3]}`;
      if (ownerForVoiceId(recordId) !== owner)
        throw new Error(`${owner}/${entry} belongs in voices/${ownerForVoiceId(recordId)}.`);
      if (replacements.has(recordId)) throw new Error(`Voice record ${recordId} appears more than once.`);
      replacements.set(recordId, bytes);
    }
  }
  return replacements;
}

function equalBytes(left: Uint8Array | undefined, right: Uint8Array | undefined) {
  return (
    !!left && !!right && left.length === right.length && left.every((byte, index) => byte === right[index])
  );
}

async function clearVoiceTree(language: DubbingLanguage) {
  await rm(resolve(languagePath(language), 'voices'), { recursive: true, force: true });
}

async function writeVoice(language: DubbingLanguage, id: string, bytes: Uint8Array) {
  const owner = ownerForVoiceId(id);
  if (!owner) throw new Error(`Cannot assign voice record ${id}.`);
  const filename = `${id.replaceAll('/', '-')}.wav`;
  const path = resolve(languagePath(language), 'voices', owner, filename);
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, bytes);
}

async function exportLanguage(language: DubbingLanguage) {
  const [originalStrings, targetStrings, originalVoice, targetVoice] = await Promise.all([
    readFile(originPath('strings.dat')),
    readFile(workspacePath(language, 'strings.dat')),
    readFile(originPath('voice.dat')),
    readFile(workspacePath(language, 'voice.dat'))
  ]);
  const originalRecords = parseStrings(originalStrings);
  const targetById = new Map(parseStrings(targetStrings).map((record) => [record.id, sourceText(record)]));
  const translations = new Map<string, string>();
  for (const record of originalRecords) {
    const target = targetById.get(record.id);
    if (target !== undefined && target !== sourceText(record)) translations.set(record.id, target);
  }
  await writeDialogue(language, translations);
  await clearVoiceTree(language);
  const originalArchive = parseVoiceArchive(originalVoice);
  const targetArchive = parseVoiceArchive(targetVoice);
  for (const original of originalArchive.records) {
    const target = targetArchive.records.find((record) => record.id === original.id);
    if (!target) throw new Error(`Target voice.dat has no record ${original.id}.`);
    const before = decodeVoiceRecord(originalArchive, original);
    const after = decodeVoiceRecord(targetArchive, target);
    if (!equalBytes(before, after) && after) await writeVoice(language, original.id, after);
  }
  await writeManifest(language, originalStrings, originalVoice);
  console.log(`Exported ${translations.size} dialogue records to dubbing/${language}.`);
}

async function checkLanguage(language: DubbingLanguage) {
  const [originalStrings, originalVoice, manifest] = await Promise.all([
    readFile(originPath('strings.dat')),
    readFile(originPath('voice.dat')),
    readManifest(language)
  ]);
  const expectedStrings = Array.isArray(manifest.stringsSha256)
    ? manifest.stringsSha256
    : [manifest.stringsSha256].filter(Boolean);
  const expectedVoice = Array.isArray(manifest.voiceSha256)
    ? manifest.voiceSha256
    : [manifest.voiceSha256].filter(Boolean);
  if (expectedStrings.length && !expectedStrings.includes(await sha256(originalStrings)))
    throw new Error(`dubbing/${language} belongs to a different strings.dat base.`);
  if (expectedVoice.length && !expectedVoice.includes(await sha256(originalVoice)))
    throw new Error(`dubbing/${language} belongs to a different voice.dat base.`);
  const recordIds = new Set(parseStrings(originalStrings).map((record) => record.id));
  for (const id of (await readDialogue(language)).keys()) {
    if (!recordIds.has(id)) throw new Error(`dubbing/${language} refers to missing dialogue record ${id}.`);
  }
  const voiceIds = new Set(parseVoiceArchive(originalVoice).records.map((record) => record.id));
  for (const id of (await voiceFiles(language)).keys()) {
    if (!voiceIds.has(id)) throw new Error(`dubbing/${language} refers to missing voice record ${id}.`);
  }
}

async function syncLanguage(language: DubbingLanguage) {
  await checkLanguage(language);
  const [originalStrings, originalVoice, translations, voices] = await Promise.all([
    readFile(originPath('strings.dat')),
    readFile(originPath('voice.dat')),
    readDialogue(language),
    voiceFiles(language)
  ]);
  const target = resolve(localGame, language);
  if (!(await exists(target))) throw new Error(`Missing Studio workspace ${target}. Run make setup first.`);
  const rebuiltStrings = rebuildStrings(
    originalStrings,
    parseStrings(originalStrings),
    Object.fromEntries(translations)
  );
  const rebuiltVoice = rebuildVoiceArchive(parseVoiceArchive(originalVoice), voices);
  await Promise.all([
    writeFile(workspacePath(language, 'strings.dat'), rebuiltStrings),
    writeFile(workspacePath(language, 'voice.dat'), rebuiltVoice)
  ]);
  console.log(
    `Synced dubbing/${language}: ${translations.size} dialogue records, ${voices.size} voice records.`
  );
}

async function organizeLanguage(language: DubbingLanguage) {
  const translations = await readDialogue(language);
  await writeDialogue(language, translations);
  await checkLanguage(language);
  console.log(`Organized dubbing/${language}.`);
}

async function writeCatalogue() {
  const languages = {} as Record<
    DubbingLanguage,
    { records: object[]; voiceIds: string[]; dubbedVoiceIds: string[] }
  >;
  for (const language of ['english', 'vietnamese'] as const) {
    const [bytes, voiceBytes, dubbedVoices] = await Promise.all([
      readFile(originPath('strings.dat')),
      readFile(originPath('voice.dat')),
      voiceFiles(language)
    ]);
    const voice = parseVoiceArchive(voiceBytes);
    const translations = await readDialogue(language);
    languages[language] = {
      voiceIds: voice.records
        .filter((record) => record.storage !== 'empty' && record.path[1] !== 3)
        .map((record) => record.id),
      dubbedVoiceIds: [...dubbedVoices.keys()].sort(compareRecordIds),
      records: parseStrings(bytes).map((record) => {
        const gadget = gadgetMetadataForStringId(record.id);
        const dialogueVoice = dialogueVoicePath(record.path[0], record.path[1], voice.bankCounts);
        return {
          id: record.id,
          owner: ownerForStringId(record.id)!,
          ...(translations.get(record.id) ? { translation: translations.get(record.id) } : {}),
          ...(dialogueVoice
            ? { voiceId: dialogueVoice.map((part) => String(part).padStart(3, '0')).join('/') }
            : {}),
          ...(gadget ? { gadgetAssetId: gadget.assetId, gadgetVoiceSlot: gadget.voiceSlot } : {})
        };
      })
    };
  }
  const catalogue = {
    format: DUBBING_FORMAT,
    generatedAt: new Date().toISOString(),
    // Public metadata only: hashes permit local browser verification without
    // distributing the original resource archives.
    fingerprints: { strings: SUPPORTED_STRINGS_HASHES, voice: SUPPORTED_VOICE_HASHES },
    languages
  };
  await writeFile(
    generatedCatalogue,
    `// Generated by bun run dubbing:catalogue. Do not edit by hand.\nexport default ${JSON.stringify(catalogue, null, 2)} as const;\n`
  );
  console.log(`Wrote ${generatedCatalogue}.`);
}

const [command, requestedLanguage] = process.argv.slice(2);
if (!command) usage();
const language = requestedLanguage && isDubbingLanguage(requestedLanguage) ? requestedLanguage : undefined;
if (requestedLanguage && !language) usage();

if (command === 'catalogue') await writeCatalogue();
else if (command === 'organize' && language) await organizeLanguage(language);
else if (command === 'check' && language) await checkLanguage(language);
else if (command === 'export' && language) await exportLanguage(language);
else if (command === 'sync' && language) await syncLanguage(language);
else if ((command === 'organize' || command === 'check') && !language) {
  for (const current of ['english', 'vietnamese'] as const) {
    if (await exists(languagePath(current))) {
      if (command === 'organize') await organizeLanguage(current);
      else await checkLanguage(current);
    }
  }
} else usage();

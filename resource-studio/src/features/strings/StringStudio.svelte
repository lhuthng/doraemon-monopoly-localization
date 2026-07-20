<script lang="ts">
  import { onMount } from 'svelte';
  import { binaryBlob, downloadBlob } from '../../lib/browser-download';
  import StudioHeader from '../../lib/components/StudioHeader.svelte';
  import { parseStrings, parseSysFont, rebuildStrings, type StringRecord } from '../../lib/formats';
  import {
    assertCompatibleVoiceArchives,
    normalizeAudioFile,
    decodeVoiceRecord,
    parseVoiceArchive,
    parseWav,
    rebuildVoiceArchive
  } from '../../lib/voice-formats';
  import { CHIFONT_MAP } from './chifont-map';
  import FindReplace from './components/FindReplace.svelte';
  import CharacterVoiceLibrary from './components/CharacterVoiceLibrary.svelte';
  import GroupNavigator from './components/GroupNavigator.svelte';
  import NonDialogueVoiceLibrary from './components/NonDialogueVoiceLibrary.svelte';
  import SharedVoiceLine from './components/SharedVoiceLine.svelte';
  import TranslationResourceMenu from './components/TranslationResourceMenu.svelte';
  import TranslationRecord from './components/TranslationRecord.svelte';
  import TranslationControls from './components/TranslationControls.svelte';
  import VoiceEditor from './components/VoiceEditor.svelte';
  import { STRING_GROUPS } from './groups';
  import { reflowGameText } from './text-layout';
  import { prepareModel, translateLine } from './translation-server-client';
  import {
    dialogueVoicePath,
    globalActionVoiceSlot,
    manifestFromArchives,
    type PreparedVoiceRecord,
    type VoiceManifest
  } from './voice';

  type TranslationFile = {
    game: string;
    source: string;
    translations: { id: string; source: string; translation: string; origin?: string }[];
  };

  type TargetLanguage = 'en' | 'vi';
  type ModelId = 'nllb' | 'm2m100';
  type TranslationOrigin = 'generated' | 'manual' | 'imported';
  type TranslationMeta = { origin: TranslationOrigin; updatedAt: number };

  const MODELS: { id: ModelId; label: string }[] = [
    { id: 'nllb', label: 'NLLB 200 distilled 600M' },
    { id: 'm2m100', label: 'M2M100 418M' }
  ];

  const TARGET_LANGUAGES: { code: TargetLanguage; label: string; cleanup: string }[] = [
    { code: 'en', label: 'English', cleanup: 'server punctuation cleanup' },
    { code: 'vi', label: 'Vietnamese', cleanup: 'supported Vietnamese glyphs + Doraemon terms' }
  ];

  let records = $state<StringRecord[]>([]);
  let archiveBytes = $state<Uint8Array | null>(null);
  let sourceName = $state('');
  let loadError = $state('');
  let search = $state('');
  let group = $state('all');
  let translations = $state<Record<string, string>>({});
  let translationMeta = $state<Record<string, TranslationMeta>>({});
  let copied = $state('');
  let exportStatus = $state('');
  let translationRunning = $state(false);
  let translationProgress = $state(0);
  let translationStage = $state('');
  let targetLanguage = $state<TargetLanguage>('en');
  let selectedModel = $state<ModelId>('nllb');
  let translationSettingsOpen = $state(false);
  let replaceDrawerOpen = $state(false);
  let queuedRecordIds = $state<string[]>([]);
  let activeRecordId = $state('');
  let generateFrom = $state('');
  let generateTo = $state('');
  let generationPaused = $state(false);
  let stopRequested = $state(false);
  let queueGoal = $state(0);
  let queueDone = $state(0);
  let replaceFind = $state('');
  let replaceWith = $state('');
  let sysfontWidths = $state<number[] | undefined>();
  let voiceManifest = $state<VoiceManifest | null>(null);
  let originalVoiceBytes = $state<Uint8Array | null>(null);
  let workingVoiceBytes = $state<Uint8Array | null>(null);
  let voiceReplacements = $state<Record<string, Uint8Array | null>>({});
  let voiceReplacementUrls = $state<Record<string, string>>({});
  let voiceReplacementDurations = $state<Record<string, number>>({});
  let voiceStatus = $state('');

  onMount(() => {
    void loadOptionalOriginal();
    void loadOptionalSysfont();
    void loadOptionalVoice();
  });

  let selectedTarget = $derived(TARGET_LANGUAGES.find((language) => language.code === targetLanguage)!);
  let queuedRecordSet = $derived(new Set(queuedRecordIds));
  let availableGroupIds = $derived(
    STRING_GROUPS.filter((item) => records.some((record) => record.path[0] === Number(item.id))).map(
      (item) => item.id
    )
  );
  let originalVoiceById = $derived(
    new Map(voiceManifest?.original.records.map((record) => [record.id, record]) ?? [])
  );
  let workingVoiceById = $derived(
    new Map(voiceManifest?.working.records.map((record) => [record.id, record]) ?? [])
  );

  let visibleRecords = $derived.by(() => {
    const query = search.trim().toLocaleLowerCase();
    return records.filter((record) => {
      if (record.path[0] !== Number(group)) return false;
      if (!query) return true;
      const text = sourceText(record).toLocaleLowerCase();
      return (
        record.id.includes(query) ||
        text.includes(query) ||
        (translations[record.id] || '').toLocaleLowerCase().includes(query)
      );
    });
  });
  let voiceOnlyRecords = $derived.by(() => {
    if (!voiceManifest || group === 'all') return [] as PreparedVoiceRecord[];
    const groupIndex = Number(group);
    if (groupIndex < 3 || groupIndex > 8) return [] as PreparedVoiceRecord[];
    return voiceManifest.working.records.filter(
      (record) =>
        record.path[0] === groupIndex - 3 &&
        record.path[1] > 0 &&
        record.path[1] !== 3 &&
        originalVoiceById.get(record.id)?.storage !== 'empty'
    );
  });
  let globalVoiceReferences = $derived.by(() => {
    if (!voiceManifest || group !== '000') return [] as PreparedVoiceRecord[];
    return voiceManifest.working.records.filter(
      (record) =>
        record.path[1] === 1 && record.path[2] < 28 && originalVoiceById.get(record.id)?.storage !== 'empty'
    );
  });
  let globalVoiceLineGroups = $derived.by(() => {
    const groups = new Map<string, PreparedVoiceRecord[]>();
    for (const voice of globalVoiceReferences) {
      const key = `${voice.path[1]}/${voice.path[2]}`;
      const line = groups.get(key) ?? [];
      line.push(voice);
      groups.set(key, line);
    }
    return [...groups.values()].sort((left, right) => left[0].path[2] - right[0].path[2]);
  });
  let remainingVisibleCount = $derived(visibleRecords.filter((record) => shouldGenerate(record)).length);

  let usedGlyphs = $derived(
    new Set(
      records.flatMap((record) =>
        record.tokens.filter((token) => token.type === 'glyph').map((token) => token.id)
      )
    )
  );
  let missingGlyphs = $derived([...usedGlyphs].filter((id) => !CHIFONT_MAP[id]).sort((a, b) => a - b));
  let translatedCount = $derived(records.filter((record) => translations[record.id]?.trim()).length);
  let manualCount = $derived(
    records.filter((record) => translations[record.id]?.trim() && translationOrigin(record.id) === 'manual')
      .length
  );
  let replacementMatches = $derived.by(() => {
    const needle = replaceFind;
    if (!needle) return [] as { id: string; start: number }[];
    const matches: { id: string; start: number }[] = [];
    for (const record of records) {
      if (isLockedForQueue(record.id)) continue;
      const text = translations[record.id] || '';
      for (
        let start = text.indexOf(needle);
        start !== -1;
        start = text.indexOf(needle, start + needle.length)
      ) {
        matches.push({ id: record.id, start });
      }
    }
    return matches;
  });

  function sourceText(record: StringRecord) {
    let text = '';
    for (const token of record.tokens) {
      if (token.type === 'glyph') text += CHIFONT_MAP[token.id] || `⟦g${token.id}⟧`;
      else if (token.type === 'vietnamese') text += token.text;
      else if (token.type === 'ascii') text += token.text;
      else if (token.type === 'newline') text += '\n';
    }
    return text;
  }

  function linkedVoice(record: StringRecord) {
    if (!voiceManifest) return undefined;
    const path = dialogueVoicePath(record.path[0], record.path[1], voiceManifest.original.bankCounts);
    if (!path) return undefined;
    const id = path.map((part) => String(part).padStart(3, '0')).join('/');
    const original = originalVoiceById.get(id);
    const working = workingVoiceById.get(id);
    if (original?.storage === 'empty') return undefined;
    return working;
  }

  function actionVoiceLine(record: StringRecord) {
    const voiceSlot = globalActionVoiceSlot(record.path[0], record.path[1]);
    if (voiceSlot === undefined) return undefined;
    return globalVoiceLineGroups.find((line) => line[0].path[2] === voiceSlot);
  }

  /**
   * Version 1.18 stores a fourth, 37-slot voice bank that looks like a late
   * dialogue continuation. The retail executable's loader explicitly stops at
   * bank 2, so these clips are archival content: editable and exportable, but
   * never selected by the unmodified game. The dialogue text itself is not
   * marked unused.
   */
  function isArchivedUnusedVoice(record: StringRecord) {
    if (!voiceManifest || record.path[0] < 3 || record.path[0] > 8) return false;
    const path = dialogueVoicePath(record.path[0], record.path[1], voiceManifest.original.bankCounts);
    return path?.[1] === 3;
  }

  function isVoiceModified(id: string) {
    if (Object.prototype.hasOwnProperty.call(voiceReplacements, id)) return true;
    return originalVoiceById.get(id)?.hash !== workingVoiceById.get(id)?.hash;
  }

  function firstLine(text: string | undefined) {
    return text?.split(/\r?\n/, 1)[0]?.trim() || undefined;
  }

  function voiceOnlyDetails(voice: PreparedVoiceRecord) {
    const bank = voice.path[1];
    const slot = voice.path[2];
    const sharedId = `00*/001/${String(slot).padStart(3, '0')}`;
    if (bank === 1 && slot <= 10) return { category: 'Menu', detail: sharedId };
    if (bank === 1 && slot <= 15)
      return {
        category: 'Actions',
        detail: `${sharedId} → 000/${String(slot + 20).padStart(3, '0')}`
      };
    if (bank === 1 && slot < 28) return { category: 'Misc', detail: sharedId };
    if (bank === 1 && slot < 64) {
      const symbol = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'[slot - 28];
      return { category: 'Alphabet', detail: `Spoken symbol ${symbol}`, symbol };
    }
    if (bank === 2) {
      const gadget = records.find((record) => record.path[0] === 1 && record.path[1] === slot);
      return {
        category: 'Gadget',
        detail: `Gadget voice ${slot}`,
        originalName: gadget ? firstLine(sourceText(gadget)) : undefined,
        translatedName: gadget ? firstLine(translations[gadget.id]) : undefined
      };
    }
    return { category: 'Additional', detail: `Unclassified bank ${bank}, slot ${slot}` };
  }

  async function replaceVoice(id: string, file: File) {
    loadError = '';
    voiceStatus = `Converting ${file.name}…`;
    try {
      if (originalVoiceById.get(id)?.storage === 'empty')
        throw new Error(`Voice slot ${id} is an empty structural placeholder and cannot be replaced.`);
      const wav = await normalizeAudioFile(file);
      const info = parseWav(wav);
      const previous = voiceReplacementUrls[id];
      if (previous) URL.revokeObjectURL(previous);
      voiceReplacements = { ...voiceReplacements, [id]: wav };
      voiceReplacementUrls = {
        ...voiceReplacementUrls,
        [id]: URL.createObjectURL(binaryBlob(wav, 'audio/wav'))
      };
      voiceReplacementDurations = { ...voiceReplacementDurations, [id]: info.duration };
      voiceStatus = `Voice ${id} replaced with ${file.name}.`;
    } catch (error) {
      loadError = `${file.name}: ${error instanceof Error ? error.message : String(error)}`;
      voiceStatus = '';
    }
  }

  async function loadVoicePlayback(id: string, source: 'original' | 'working') {
    try {
      let bytes = source === 'original' ? originalVoiceBytes : workingVoiceBytes;
      if (!bytes) {
        const filename = source === 'original' ? 'voice-origin.dat' : 'voice.dat';
        const response = await fetch(`/game/${filename}`);
        if (!response.ok) throw new Error(`Cannot load ${filename}.`);
        bytes = new Uint8Array(await response.arrayBuffer());
        if (source === 'original') originalVoiceBytes = bytes;
        else workingVoiceBytes = bytes;
      }
      const archive = parseVoiceArchive(bytes);
      const record = archive.records.find((candidate) => candidate.id === id);
      if (!record) throw new Error(`voice.dat has no slot ${id}.`);
      const wav = decodeVoiceRecord(archive, record);
      if (!wav) throw new Error(`Voice slot ${id} is empty.`);
      const info = parseWav(wav);
      const url = URL.createObjectURL(binaryBlob(wav, 'audio/wav'));
      if (!voiceManifest) return;
      const nextSource = voiceManifest[source];
      voiceManifest = {
        ...voiceManifest,
        [source]: {
          ...nextSource,
          records: nextSource.records.map((item) =>
            item.id === id
              ? {
                  ...item,
                  url,
                  duration: info.duration,
                  sampleRate: info.sampleRate,
                  bitsPerSample: info.bitsPerSample
                }
              : item
          )
        }
      };
    } catch (error) {
      loadError = error instanceof Error ? error.message : String(error);
    }
  }

  async function restoreOriginalVoice(id: string) {
    try {
      const original = originalVoiceById.get(id);
      const working = workingVoiceById.get(id);
      const previous = voiceReplacementUrls[id];
      if (previous) URL.revokeObjectURL(previous);
      const nextUrls = { ...voiceReplacementUrls };
      delete nextUrls[id];
      voiceReplacementUrls = nextUrls;
      const nextDurations = { ...voiceReplacementDurations };
      delete nextDurations[id];
      voiceReplacementDurations = nextDurations;
      if (original?.hash === working?.hash && original?.storage === working?.storage) {
        const next = { ...voiceReplacements };
        delete next[id];
        voiceReplacements = next;
      } else if (!original?.url) {
        voiceReplacements = { ...voiceReplacements, [id]: null };
      } else {
        const response = await fetch(original.url);
        if (!response.ok) throw new Error(`Cannot load original voice ${id}.`);
        const wav = new Uint8Array(await response.arrayBuffer());
        voiceReplacements = { ...voiceReplacements, [id]: wav };
        voiceReplacementUrls = {
          ...voiceReplacementUrls,
          [id]: URL.createObjectURL(binaryBlob(wav, 'audio/wav'))
        };
      }
      voiceStatus = `Voice ${id} restored to the original recording.`;
    } catch (error) {
      loadError = error instanceof Error ? error.message : String(error);
    }
  }

  async function loadOptionalVoice() {
    try {
      const response = await fetch('/game/prepared/voice/manifest.json');
      if (response.ok) voiceManifest = await response.json();
    } catch {
      /* Voice files are optional outside a staged private workspace. */
    }
  }

  async function loadOriginalVoice(file: Blob, name: string) {
    loadError = '';
    voiceStatus = `Reading ${name}…`;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      parseVoiceArchive(bytes);
      originalVoiceBytes = bytes;
      workingVoiceBytes = bytes;
      voiceManifest = await manifestFromArchives(bytes, name);
      voiceReplacements = {};
      voiceReplacementUrls = {};
      voiceReplacementDurations = {};
      voiceStatus = `Loaded ${voiceManifest.working.records.length} voice slots from ${name}.`;
    } catch (error) {
      loadError = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  async function loadModifiedVoice(file: Blob, name: string) {
    loadError = '';
    voiceStatus = `Comparing ${name} with the original voice archive…`;
    try {
      let original = originalVoiceBytes;
      if (!original) {
        const response = await fetch('/game/voice-origin.dat');
        if (!response.ok) throw new Error('Load the original voice.dat first.');
        original = new Uint8Array(await response.arrayBuffer());
        originalVoiceBytes = original;
      }
      const working = new Uint8Array(await file.arrayBuffer());
      assertCompatibleVoiceArchives(parseVoiceArchive(original), parseVoiceArchive(working));
      workingVoiceBytes = working;
      const nextManifest = await manifestFromArchives(original, 'voice-origin.dat', working, name);
      voiceManifest = nextManifest;
      voiceReplacements = {};
      voiceReplacementUrls = {};
      voiceReplacementDurations = {};
      const originalById = new Map(nextManifest.original.records.map((record) => [record.id, record]));
      const changed = nextManifest.working.records.filter(
        (record) =>
          record.hash !== originalById.get(record.id)?.hash ||
          record.storage !== originalById.get(record.id)?.storage
      ).length;
      voiceStatus = `Loaded ${name}; ${changed} slots differ from the original.`;
    } catch (error) {
      loadError = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  async function originalVoiceInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) await loadOriginalVoice(input.files[0], input.files[0].name);
    input.value = '';
  }

  async function modifiedVoiceInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) await loadModifiedVoice(input.files[0], input.files[0].name);
    input.value = '';
  }

  async function exportVoiceDat() {
    loadError = '';
    voiceStatus = 'Rebuilding and verifying voice.dat…';
    try {
      let bytes = workingVoiceBytes;
      if (!bytes) {
        const response = await fetch('/game/voice.dat');
        if (!response.ok) throw new Error('Load a working voice.dat before exporting.');
        bytes = new Uint8Array(await response.arrayBuffer());
      }
      const replacements = new Map(Object.entries(voiceReplacements));
      const rebuilt = rebuildVoiceArchive(parseVoiceArchive(bytes), replacements);
      downloadBlob(binaryBlob(rebuilt), 'voice.dat');
      voiceStatus = `Exported voice.dat with ${replacements.size} session changes.`;
    } catch (error) {
      loadError = error instanceof Error ? error.message : String(error);
      voiceStatus = '';
    }
  }

  function selectGroup(id: string) {
    group = id;
    search = '';
  }

  function scrollToElement(id: string) {
    window.requestAnimationFrame(() =>
      document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'center' })
    );
  }

  function openCharacterVoice(voice: PreparedVoiceRecord) {
    selectGroup(String(voice.path[0] + 3).padStart(3, '0'));
    scrollToElement(`voice-${voice.id}`);
  }

  function replacementRecordId(record: StringRecord) {
    return `record-${record.id.replace('/', '-')}`;
  }

  function showReplacement(find: string, index: number) {
    const matches = replacementMatches;
    if (!matches.length) return;
    const resolved = ((index % matches.length) + matches.length) % matches.length;
    const match = matches[resolved];
    const record = recordById(match.id);
    if (!record) return;
    selectGroup(String(record.path[0]).padStart(3, '0'));
    window.requestAnimationFrame(() => {
      const article = document.getElementById(replacementRecordId(record));
      article?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      const field = article?.querySelector('textarea');
      field?.focus();
      field?.setSelectionRange(match.start, match.start + find.length);
    });
  }

  function replaceOne(find: string, replacement: string, index: number) {
    const matches = replacementMatches;
    if (!matches.length) return;
    const current = index < 0 ? 0 : Math.min(index, matches.length - 1);
    const match = matches[current];
    const text = translations[match.id] || '';
    if (text.slice(match.start, match.start + find.length) !== find) return;
    saveTranslation(
      match.id,
      text.slice(0, match.start) + replacement + text.slice(match.start + find.length)
    );
  }

  function replaceAll(find: string, replacement: string) {
    for (const record of records) {
      if (isLockedForQueue(record.id)) continue;
      const text = translations[record.id] || '';
      if (!text.includes(find)) continue;
      saveTranslation(record.id, text.replaceAll(find, replacement));
    }
  }

  function reflowTranslation(record: StringRecord, width: number, preset: 'gadgets' | 'dialog') {
    const original = translations[record.id];
    if (!original?.trim() || isLockedForQueue(record.id)) return;
    if (!sysfontWidths) {
      loadError = 'sysfont.dat is still loading; try reflow again in a moment.';
      return;
    }
    const result = reflowGameText(original, width, sysfontWidths, false);
    saveTranslation(record.id, result.text);
    exportStatus = result.oversizedWords.length
      ? `Reflowed ${record.id} to ${width}px. These words are wider than the box: ${[...new Set(result.oversizedWords)].join(', ')}.`
      : `Reflowed ${record.id} to ${width}px using ${preset === 'dialog' ? 'Dialog' : 'Gadgets'} sysfont measurements. Capitalization was left unchanged.`;
  }

  function flattenTranslation(record: StringRecord) {
    const original = translations[record.id];
    if (original === undefined || isLockedForQueue(record.id)) return;
    const flattened = original.replace(/\r\n?|\n/g, ' ').replace(/[ \t]{2,}/g, ' ');
    saveTranslation(record.id, flattened);
    exportStatus = `Flattened line breaks in ${record.id}.`;
  }

  async function loadArchive(file: Blob, name: string) {
    loadError = '';
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const parsed = parseStrings(bytes);
      archiveBytes = bytes;
      records = parsed;
      sourceName = name;
      search = '';
      group = '000';
      const saved = localStorage.getItem('doraemon-translations');
      if (saved) translations = JSON.parse(saved);
      const savedMeta = localStorage.getItem('doraemon-translation-meta');
      if (savedMeta) translationMeta = JSON.parse(savedMeta);
      else
        translationMeta = Object.fromEntries(
          Object.keys(translations).map((id) => [id, { origin: 'manual', updatedAt: Date.now() }])
        );
      localStorage.removeItem('doraemon-rough-translations');
    } catch (error) {
      loadError = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  async function loadSysfont(file: Blob, name: string) {
    try {
      const font = parseSysFont(new Uint8Array(await file.arrayBuffer()));
      sysfontWidths = font.glyphs.slice(0, 128).map((glyph) => glyph.width);
    } catch (error) {
      loadError = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  async function resetAndLoadOriginal(file: Blob, name: string) {
    loadError = '';
    translations = {};
    translationMeta = {};
    localStorage.removeItem('doraemon-translations');
    localStorage.removeItem('doraemon-translation-meta');
    await loadArchive(file, name);
  }

  async function loadOptionalOriginal() {
    try {
      const response = await fetch('/game/strings-origin.dat');
      if (response.ok) await resetAndLoadOriginal(await response.blob(), 'strings-origin.dat');
    } catch {
      /* Optional local development file. */
    }
    try {
      const response = await fetch('/game/strings.dat');
      if (response.ok && records.length) await importTranslationArchive(await response.blob(), 'strings.dat');
    } catch {
      /* No previously modified strings.dat to import. */
    }
  }

  async function loadOptionalSysfont() {
    try {
      const response = await fetch('/game/sysfont.dat');
      if (response.ok) await loadSysfont(await response.blob(), 'sysfont.dat');
    } catch {
      /* Reflow remains unavailable until the user loads a font. */
    }
  }

  async function originalInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) await resetAndLoadOriginal(input.files[0], input.files[0].name);
    input.value = '';
  }

  async function sysfontInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) await loadSysfont(input.files[0], input.files[0].name);
    input.value = '';
  }

  function dropOriginal(event: DragEvent) {
    event.preventDefault();
    const files = Array.from(event.dataTransfer?.files ?? []);
    const stringsOrigin = files.find((file) => file.name.toLowerCase() === 'strings-origin.dat');
    const stringsModified = !stringsOrigin && files.find((file) => file.name.toLowerCase() === 'strings.dat');
    const sysfont = files.find((file) => file.name.toLowerCase() === 'sysfont.dat');
    if (stringsOrigin) void resetAndLoadOriginal(stringsOrigin, stringsOrigin.name);
    else if (stringsModified) void importTranslationArchive(stringsModified, stringsModified.name);
    if (sysfont) void loadSysfont(sysfont, sysfont.name);
    if (!stringsOrigin && !stringsModified && !sysfont)
      loadError = 'Drop strings-origin.dat, strings.dat, or sysfont.dat here.';
  }

  async function importTranslationArchive(file: Blob, name: string) {
    loadError = '';
    exportStatus = '';
    try {
      if (!records.length || !archiveBytes)
        throw new Error('Load the original strings-origin.dat before importing a translated .dat file.');
      const importedRecords = parseStrings(new Uint8Array(await file.arrayBuffer()));
      const importedById = new Map(importedRecords.map((record) => [record.id, sourceText(record)]));
      const now = Date.now();
      const nextTranslations = { ...translations };
      const nextMeta = { ...translationMeta };
      let matched = 0;
      let changed = 0;
      let unchanged = 0;

      for (const original of records) {
        const importedText = importedById.get(original.id);
        if (importedText === undefined) continue;
        matched += 1;
        if (importedText === sourceText(original)) {
          delete nextTranslations[original.id];
          delete nextMeta[original.id];
          unchanged += 1;
        } else {
          nextTranslations[original.id] = importedText;
          nextMeta[original.id] = { origin: 'imported', updatedAt: now };
          changed += 1;
        }
      }

      if (!matched) throw new Error('No matching record IDs were found in this .dat file.');
      translations = nextTranslations;
      translationMeta = nextMeta;
      localStorage.setItem('doraemon-translations', JSON.stringify(translations));
      localStorage.setItem('doraemon-translation-meta', JSON.stringify(translationMeta));
      const ignored = importedRecords.length - matched;
      exportStatus = `Imported ${name}: ${changed} translated records filled; ${unchanged} records match the original and remain blank${ignored ? `; ${ignored} unmatched records ignored` : ''}.`;
    } catch (error) {
      loadError = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  async function translatedArchiveInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) await importTranslationArchive(input.files[0], input.files[0].name);
    input.value = '';
  }

  function translationOrigin(id: string) {
    return translationMeta[id]?.origin;
  }

  function shouldGenerate(record: StringRecord) {
    return !translations[record.id]?.trim();
  }

  function generationState(record: StringRecord) {
    if (activeRecordId === record.id) return 'Translating';
    if (queuedRecordSet.has(record.id)) return 'Queued';
    if (!translations[record.id]?.trim()) return 'Keep';
    const origin = translationOrigin(record.id);
    if (origin === 'manual') return 'Translated manually';
    return 'Translated';
  }

  function untranslated(recordsToCheck: StringRecord[]) {
    return recordsToCheck.filter((record) => shouldGenerate(record));
  }

  function translationSegments(sourceRecords: StringRecord[]) {
    const segments: { id: string; line: number; text: string }[] = [];
    const linesById = new Map<string, string[]>();
    for (const record of sourceRecords) {
      const lines = sourceText(record).split('\n');
      linesById.set(record.id, [...lines]);
      lines.forEach((text, line) => {
        if (text.trim()) segments.push({ id: record.id, line, text });
      });
    }
    return { segments, linesById };
  }

  function isLockedForQueue(id: string) {
    return activeRecordId === id || queuedRecordSet.has(id);
  }

  function saveGeneratedTranslation(id: string, value: string) {
    translations = { ...translations, [id]: value };
    translationMeta = { ...translationMeta, [id]: { origin: 'generated', updatedAt: Date.now() } };
    localStorage.setItem('doraemon-translations', JSON.stringify(translations));
    localStorage.setItem('doraemon-translation-meta', JSON.stringify(translationMeta));
  }

  async function translateLineOnServer(text: string) {
    return translateLine(selectedModel, targetLanguage, text);
  }

  async function prepareTranslationServer() {
    translationStage = `Preparing ${MODELS.find((model) => model.id === selectedModel)?.label}… queued records will wait until the server is ready.`;
    await prepareModel(selectedModel, (message) => (translationStage = message));
    translationStage = 'Translation server ready. Starting queued records…';
  }

  function recordById(id: string) {
    return records.find((record) => record.id === id);
  }

  function enqueueRecords(sourceRecords: StringRecord[], options: { front?: boolean; force?: boolean } = {}) {
    const ids = sourceRecords
      .filter((record) => options.force || shouldGenerate(record))
      .map((record) => record.id)
      .filter((id) => id !== activeRecordId);
    if (!ids.length) return 0;
    const unique = [...new Set(ids)];
    const uniqueSet = new Set(unique);
    const existingSet = new Set(queuedRecordIds);
    const rest = queuedRecordIds.filter((id) => !uniqueSet.has(id));
    queuedRecordIds = options.front ? [...unique, ...rest] : [...rest, ...unique];
    queueGoal += unique.filter((id) => !existingSet.has(id)).length;
    return unique.length;
  }

  async function translateOneRecord(record: StringRecord) {
    const { segments, linesById } = translationSegments([record]);
    for (let offset = 0; offset < segments.length; offset += 1) {
      const segment = segments[offset];
      translationStage = `Translating ${record.id} line ${offset + 1}/${segments.length} on Bun server...`;
      linesById.get(segment.id)![segment.line] = await translateLineOnServer(segment.text);
      await new Promise((resolve) => window.setTimeout(resolve, 0));
    }
    const translated = linesById.get(record.id)!.join('\n');
    saveGeneratedTranslation(record.id, translated);
    return translated;
  }

  async function processQueue() {
    if (translationRunning || generationPaused || !queuedRecordIds.length) return;
    translationRunning = true;
    stopRequested = false;
    loadError = '';
    exportStatus = '';
    try {
      await prepareTranslationServer();
      while (queuedRecordIds.length && !generationPaused && !stopRequested) {
        const [id, ...rest] = queuedRecordIds;
        queuedRecordIds = rest;
        const record = recordById(id);
        if (!record) continue;
        activeRecordId = record.id;
        await translateOneRecord(record);
        queueDone += 1;
        translationProgress = queueGoal ? Math.round((queueDone / queueGoal) * 100) : 0;
      }
      if (stopRequested) {
        queuedRecordIds = [];
        translationStage = `Stopped. ${queueDone}/${queueGoal} records generated.`;
      } else if (generationPaused) {
        translationStage = `Paused. ${queueDone}/${queueGoal} records generated.`;
      } else {
        translationStage = `Completed ${queueDone}/${queueGoal} ${selectedTarget.label} records. Cleanup: ${selectedTarget.cleanup}.`;
      }
      exportStatus = translationStage;
    } catch (error) {
      loadError = `Translation failed after saving completed records: ${error instanceof Error ? error.message : String(error)}`;
      translationStage = 'Translation stopped. Already completed records were kept.';
    } finally {
      activeRecordId = '';
      translationRunning = false;
    }
  }

  async function translateRecords(
    sourceRecords: StringRecord[],
    options: { skipCompleted?: boolean; front?: boolean; force?: boolean } = {}
  ) {
    const pending = options.skipCompleted ? untranslated(sourceRecords) : sourceRecords;
    if (!pending.length) {
      translationStage = 'Nothing to translate in the selected range.';
      exportStatus = translationStage;
      return;
    }
    const wasPaused = generationPaused && !translationRunning;
    if (!translationRunning && !queuedRecordIds.length) {
      queueGoal = 0;
      queueDone = 0;
      translationProgress = 0;
    }
    if (!wasPaused) generationPaused = false;
    stopRequested = false;
    const count = enqueueRecords(pending, { front: options.front, force: options.force });
    if (!count) return;
    translationStage = `Queued ${count} ${selectedTarget.label} records with ${MODELS.find((model) => model.id === selectedModel)?.label}.`;
    if (!wasPaused) await processQueue();
  }

  function rangeRecords() {
    const from = generateFrom.trim();
    const to = generateTo.trim();
    const start = from ? records.findIndex((record) => record.id === from) : 0;
    const end = to ? records.findIndex((record) => record.id === to) : records.length - 1;
    if (start < 0) throw new Error(`Unknown start record "${from}".`);
    if (end < 0) throw new Error(`Unknown end record "${to}".`);
    if (start > end) throw new Error('Start record must be before end record.');
    return records.slice(start, end + 1);
  }

  async function startGenerating() {
    loadError = '';
    try {
      await translateRecords(rangeRecords(), { skipCompleted: true });
    } catch (error) {
      loadError = error instanceof Error ? error.message : String(error);
    }
  }

  async function regenerateRecord(record: StringRecord) {
    await translateRecords([record], { skipCompleted: false, front: true, force: true });
  }

  function requestPause() {
    generationPaused = true;
    translationStage = 'Paused. Current record will finish first.';
  }

  async function resumeGeneration() {
    generationPaused = false;
    await processQueue();
  }

  function requestStop() {
    stopRequested = true;
    queuedRecordIds = [];
    if (generationPaused && !translationRunning) {
      generationPaused = false;
      translationStage = `Stopped. ${queueDone}/${queueGoal} records generated.`;
      exportStatus = translationStage;
    } else {
      translationStage = 'Stop requested. Current record will finish first.';
    }
  }

  function updateTranslation(id: string, value: string) {
    saveTranslation(id, value);
  }

  function saveTranslation(id: string, value: string) {
    translations = { ...translations, [id]: value };
    translationMeta = { ...translationMeta, [id]: { origin: 'manual', updatedAt: Date.now() } };
    localStorage.setItem('doraemon-translations', JSON.stringify(translations));
    localStorage.setItem('doraemon-translation-meta', JSON.stringify(translationMeta));
  }

  async function copyText(text: string, id: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const temporary = document.createElement('textarea');
      temporary.value = text;
      temporary.style.position = 'fixed';
      temporary.style.opacity = '0';
      document.body.append(temporary);
      temporary.select();
      document.execCommand('copy');
      temporary.remove();
    }
    copied = id;
    window.setTimeout(() => {
      if (copied === id) copied = '';
    }, 1200);
  }

  function copyAll() {
    const text = visibleRecords
      .map((record) => `${record.id}\t${sourceText(record).replaceAll('\n', '\\N')}`)
      .join('\n');
    copyText(text, 'all');
  }

  function exportTranslations() {
    const data: TranslationFile = {
      game: 'Doraemon Monopoly',
      source: sourceName,
      translations: records.map((record) => ({
        id: record.id,
        source: sourceText(record),
        translation: translations[record.id] || '',
        origin: translationOrigin(record.id) || ''
      }))
    };
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(data, null, 2) + '\n'], { type: 'application/json' })
    );
    const link = document.createElement('a');
    link.href = url;
    link.download = 'doraemon-translations.json';
    link.click();
    URL.revokeObjectURL(url);
  }

  function exportChineseRecords() {
    const data = {
      format: 'doraemon-chinese-records-v1',
      source: sourceName,
      records: Object.fromEntries(records.map((record) => [record.id, sourceText(record)]))
    };
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(data, null, 2) + '\n'], { type: 'application/json' })
    );
    const link = document.createElement('a');
    link.href = url;
    link.download = 'records-chinese.json';
    link.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
    exportStatus = `Exported ${records.length} Chinese records keyed by ID.`;
  }

  function exportStringsDat() {
    loadError = '';
    exportStatus = '';
    try {
      if (!archiveBytes) throw new Error('Load the original strings-origin.dat first.');
      const rebuilt = rebuildStrings(archiveBytes, records, translations);
      downloadBlob(binaryBlob(rebuilt), 'strings-exported.dat');
      exportStatus = `Built and verified strings-exported.dat: ${translatedCount} translated, ${records.length - translatedCount} preserved in Chinese.`;
    } catch (error) {
      loadError = error instanceof Error ? error.message : String(error);
    }
  }

  function clearTranslations() {
    if (!window.confirm('Clear every translation saved in this browser?')) return;
    translations = {};
    translationMeta = {};
    localStorage.removeItem('doraemon-translations');
    localStorage.removeItem('doraemon-translation-meta');
  }
</script>

<main class="translation-studio">
  <StudioHeader
    title="Translation studio"
    description="Text, voices, and game-ready exports in one workspace."
    active="translation"
    className="app-header"
  />
  <TranslationResourceMenu
    hasRecords={!!records.length}
    hasArchive={!!archiveBytes}
    hasVoice={!!voiceManifest}
    onOriginalStrings={originalInput}
    onModifiedStrings={translatedArchiveInput}
    onSysfont={sysfontInput}
    onOriginalVoice={originalVoiceInput}
    onModifiedVoice={modifiedVoiceInput}
    onExportSource={exportChineseRecords}
    onExportProject={exportTranslations}
    onExportStrings={exportStringsDat}
    onExportVoice={exportVoiceDat}
  />

  {#if !records.length}
    <section
      class="drop-zone"
      role="group"
      aria-label="Load string resources"
      ondragover={(event) => event.preventDefault()}
      ondrop={dropOriginal}
    >
      <strong>Bring your own game files</strong>
      <span
        >Drop the original <code>strings.dat</code>, then optionally a modified copy,
        <code>sysfont.dat</code>, and <code>voice.dat</code>.</span
      >
    </section>
  {/if}

  {#if loadError}<p class="error" role="alert">{loadError}</p>{/if}

  {#if records.length}
    <section class="summary" aria-label="Translation status">
      <div><span>Source</span><strong>{sourceName}</strong></div>
      <div><span>Records</span><strong>{records.length}</strong></div>
      <div>
        <span>Glyph mapping</span><strong
          >{missingGlyphs.length
            ? `${usedGlyphs.size - missingGlyphs.length}/${usedGlyphs.size}`
            : `${usedGlyphs.size}/${usedGlyphs.size} complete`}</strong
        >
      </div>
      <div><span>Translated</span><strong>{translatedCount}/{records.length}</strong></div>
    </section>

    {#if missingGlyphs.length}<p class="error">Unmapped glyph IDs: {missingGlyphs.join(', ')}</p>{/if}
    {#if exportStatus}<p class="success" role="status">{exportStatus}</p>{/if}
    {#if voiceStatus}<p class="success" role="status">{voiceStatus}</p>{/if}

    <TranslationControls
      bind:search
      bind:targetLanguage
      bind:model={selectedModel}
      bind:from={generateFrom}
      bind:to={generateTo}
      bind:settingsOpen={translationSettingsOpen}
      bind:replaceOpen={replaceDrawerOpen}
      languages={TARGET_LANGUAGES}
      models={MODELS}
      visibleCount={visibleRecords.length}
      {translatedCount}
      targetLabel={selectedTarget.label}
      running={translationRunning}
      paused={generationPaused}
      stopping={stopRequested}
      queuedCount={queuedRecordIds.length}
      progress={translationProgress}
      copiedAll={copied === 'all'}
      onStart={startGenerating}
      onResume={resumeGeneration}
      onPause={requestPause}
      onStop={requestStop}
      onCopy={copyAll}
      onClear={clearTranslations}
    />

    <div class="workspace translation-workspace">
      <div class="workspace-content">
        {#if translationRunning || translationStage}
          <section class="translation-progress" aria-live="polite">
            <progress max="100" value={translationProgress}></progress>
            <span>{translationStage}</span>
          </section>
        {/if}

        <div class="result-count">
          {visibleRecords.length} records, {remainingVisibleCount} keep/generatable in current view · {translatedCount}
          translated · {manualCount} manually edited
        </div>
        <section class="translation-list" aria-label="Decoded strings">
          {#each visibleRecords as record (record.id)}
            <TranslationRecord
              {record}
              source={sourceText(record)}
              translation={translations[record.id] || ''}
              generationState={generationState(record)}
              archived={isArchivedUnusedVoice(record)}
              queued={queuedRecordSet.has(record.id)}
              translating={activeRecordId === record.id}
              origin={translationOrigin(record.id)}
              locked={isLockedForQueue(record.id)}
              copied={copied === record.id}
              onRegenerate={() => regenerateRecord(record)}
              onReflow={(width, preset) => reflowTranslation(record, width, preset)}
              onFlatten={() => flattenTranslation(record)}
              onCopy={() => copyText(sourceText(record), record.id)}
              onTranslation={(value) => updateTranslation(record.id, value)}
            >
              {#if record.path[0] >= 3 && record.path[0] <= 8}
                {@const voice = linkedVoice(record)}
                {#if voice}
                  <VoiceEditor
                    original={originalVoiceById.get(voice.id)}
                    working={voice}
                    replacementUrl={voiceReplacementUrls[voice.id]}
                    replacementDuration={voiceReplacementDurations[voice.id]}
                    cleared={Object.prototype.hasOwnProperty.call(voiceReplacements, voice.id) &&
                      voiceReplacements[voice.id] === null}
                    modified={isVoiceModified(voice.id)}
                    onReplace={(file) => void replaceVoice(voice.id, file)}
                    onReset={() => void restoreOriginalVoice(voice.id)}
                    onLoadOriginal={() => void loadVoicePlayback(voice.id, 'original')}
                    onLoadWorking={() => void loadVoicePlayback(voice.id, 'working')}
                  />
                {/if}
              {/if}
              {#if actionVoiceLine(record)}
                <SharedVoiceLine
                  title={`${record.id} · 00*/001/${String(record.path[1] - 20).padStart(3, '0')}`}
                  voices={actionVoiceLine(record) ?? []}
                  characters={voiceManifest?.characters ?? []}
                  originalById={originalVoiceById}
                  replacementUrls={voiceReplacementUrls}
                  replacementDurations={voiceReplacementDurations}
                  replacements={voiceReplacements}
                  isModified={isVoiceModified}
                  detailsFor={voiceOnlyDetails}
                  onJump={openCharacterVoice}
                  onLoadOriginal={(id) => void loadVoicePlayback(id, 'original')}
                  onLoadWorking={(id) => void loadVoicePlayback(id, 'working')}
                />
              {/if}
            </TranslationRecord>
          {/each}
        </section>
        {#if globalVoiceReferences.length}
          <NonDialogueVoiceLibrary
            lines={globalVoiceLineGroups}
            characters={voiceManifest?.characters ?? []}
            originalById={originalVoiceById}
            replacementUrls={voiceReplacementUrls}
            replacementDurations={voiceReplacementDurations}
            replacements={voiceReplacements}
            isModified={isVoiceModified}
            detailsFor={voiceOnlyDetails}
            onJump={openCharacterVoice}
            onLoadOriginal={(id) => void loadVoicePlayback(id, 'original')}
            onLoadWorking={(id) => void loadVoicePlayback(id, 'working')}
          />
        {/if}
        {#if voiceOnlyRecords.length}
          <CharacterVoiceLibrary
            voices={voiceOnlyRecords}
            originalById={originalVoiceById}
            replacementUrls={voiceReplacementUrls}
            replacementDurations={voiceReplacementDurations}
            replacements={voiceReplacements}
            isModified={isVoiceModified}
            detailsFor={voiceOnlyDetails}
            onReplace={(id, file) => void replaceVoice(id, file)}
            onReset={(id) => void restoreOriginalVoice(id)}
            onLoadOriginal={(id) => void loadVoicePlayback(id, 'original')}
            onLoadWorking={(id) => void loadVoicePlayback(id, 'working')}
          />
        {/if}
      </div>
      <aside class="workspace-sidebar bottom-workspace-bar">
        <GroupNavigator bind:group {availableGroupIds} onNavigate={selectGroup} />
      </aside>
      <aside class:open={replaceDrawerOpen} class="replace-drawer" aria-label="Find and replace drawer">
        <div class="replace-drawer-header">
          <strong>Find &amp; replace</strong>
          <button
            type="button"
            class="quiet"
            aria-label="Close find and replace"
            onclick={() => (replaceDrawerOpen = false)}>×</button
          >
        </div>
        <FindReplace
          bind:find={replaceFind}
          bind:replacement={replaceWith}
          matches={replacementMatches}
          onShow={showReplacement}
          onReplaceOne={replaceOne}
          onReplaceAll={replaceAll}
        />
      </aside>
    </div>
  {/if}
</main>

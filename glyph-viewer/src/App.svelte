<script lang="ts">
  import { CHIFONT_MAP } from './lib/chifont-map';
  import { parseStrings, rebuildStrings, type StringRecord } from './lib/formats';
  import { STRING_GROUPS } from './lib/groups';
  import { GADGETS_LAYOUT, reflowGameText } from './lib/text-layout';
  import FindReplace from './lib/components/FindReplace.svelte';
  import GroupNavigator from './lib/components/GroupNavigator.svelte';

  type TranslationFile = {
    game: string;
    source: string;
    translations: { id: string; source: string; translation: string; origin?: string }[];
  };

  type TranslationEntry = { id?: unknown; translation?: unknown; text?: unknown };
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
    { code: 'vi', label: 'Vietnamese', cleanup: 'server ASCII without accents + Doraemon terms' }
  ];

  let records = $state<StringRecord[]>([]);
  let archiveBytes = $state<Uint8Array | null>(null);
  let sourceName = $state('');
  let loadError = $state('');
  let dragging = $state(false);
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
  let layoutWidth = $state(GADGETS_LAYOUT.maxWidth);

  let selectedTarget = $derived(TARGET_LANGUAGES.find((language) => language.code === targetLanguage)!);
  let queuedRecordSet = $derived(new Set(queuedRecordIds));
  let availableGroupIds = $derived(STRING_GROUPS.filter((item) => records.some((record) => record.path[0] === Number(item.id))).map((item) => item.id));

  let visibleRecords = $derived.by(() => {
    const query = search.trim().toLocaleLowerCase();
    return records.filter((record) => {
      if (group !== 'all' && record.path[0] !== Number(group)) return false;
      if (!query) return true;
      const text = sourceText(record).toLocaleLowerCase();
      return record.id.includes(query) || text.includes(query) || (translations[record.id] || '').toLocaleLowerCase().includes(query);
    });
  });
  let remainingVisibleCount = $derived(visibleRecords.filter((record) => shouldGenerate(record)).length);

  let usedGlyphs = $derived(new Set(records.flatMap((record) => record.tokens.filter((token) => token.type === 'glyph').map((token) => token.id))));
  let missingGlyphs = $derived([...usedGlyphs].filter((id) => !CHIFONT_MAP[id]).sort((a, b) => a - b));
  let translatedCount = $derived(records.filter((record) => translations[record.id]?.trim()).length);
  let manualCount = $derived(records.filter((record) => translations[record.id]?.trim() && translationOrigin(record.id) === 'manual').length);
  let replacementMatches = $derived.by(() => {
    const needle = replaceFind;
    if (!needle) return [] as { id: string; start: number }[];
    const matches: { id: string; start: number }[] = [];
    for (const record of records) {
      if (isLockedForQueue(record.id)) continue;
      const text = translations[record.id] || '';
      for (let start = text.indexOf(needle); start !== -1; start = text.indexOf(needle, start + needle.length)) {
        matches.push({ id: record.id, start });
      }
    }
    return matches;
  });

  function sourceText(record: StringRecord) {
    let text = '';
    for (const token of record.tokens) {
      if (token.type === 'glyph') text += CHIFONT_MAP[token.id] || `⟦g${token.id}⟧`;
      else if (token.type === 'ascii') text += token.text;
      else if (token.type === 'newline') text += '\n';
    }
    return text;
  }

  function selectGroup(id: string) {
    group = id;
    search = '';
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
    saveTranslation(match.id, text.slice(0, match.start) + replacement + text.slice(match.start + find.length));
  }

  function replaceAll(find: string, replacement: string) {
    for (const record of records) {
      if (isLockedForQueue(record.id)) continue;
      const text = translations[record.id] || '';
      if (!text.includes(find)) continue;
      saveTranslation(record.id, text.replaceAll(find, replacement));
    }
  }

  function reflowTranslation(record: StringRecord) {
    const original = translations[record.id];
    if (!original?.trim() || isLockedForQueue(record.id)) return;
    const result = reflowGameText(original, layoutWidth);
    saveTranslation(record.id, result.text);
    exportStatus = result.oversizedWords.length
      ? `Reflowed ${record.id} to ${layoutWidth}px. These words are wider than the box: ${[...new Set(result.oversizedWords)].join(', ')}.`
      : `Reflowed ${record.id} to ${layoutWidth}px using Gadgets sysfont measurements. Capitalization was left unchanged.`;
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
      group = 'all';
      const saved = localStorage.getItem('doraemon-translations');
      if (saved) translations = JSON.parse(saved);
      const savedMeta = localStorage.getItem('doraemon-translation-meta');
      if (savedMeta) translationMeta = JSON.parse(savedMeta);
      else translationMeta = Object.fromEntries(Object.keys(translations).map((id) => [id, { origin: 'manual', updatedAt: Date.now() }]));
      localStorage.removeItem('doraemon-rough-translations');
    } catch (error) {
      loadError = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  function importTranslationDocument(document: unknown) {
    if (!document || typeof document !== 'object') throw new Error('Translation JSON must contain an object.');
    const imported: Record<string, string> = {};
    const value = document as Record<string, unknown>;
    if (Array.isArray(value.translations)) {
      for (const entry of value.translations as TranslationEntry[]) {
        const text = typeof entry.translation === 'string' ? entry.translation : entry.text;
        if (typeof entry.id !== 'string' || typeof text !== 'string') throw new Error('Translation JSON contains an invalid array record.');
        imported[entry.id] = text;
      }
    } else {
      const mapping = value.translations && typeof value.translations === 'object' ? value.translations as Record<string, unknown> : value;
      for (const [id, text] of Object.entries(mapping)) {
        if (/^\d{3}\/\d{3}$/.test(id) && typeof text === 'string') imported[id] = text;
      }
    }
    const ids = new Set(records.map((record) => record.id));
    const entries = Object.entries(imported).filter(([id]) => ids.has(id));
    if (!entries.length) throw new Error('No matching record IDs were found. Expected keys such as "000/000".');
    translations = { ...translations, ...Object.fromEntries(entries) };
    translationMeta = {
      ...translationMeta,
      ...Object.fromEntries(entries.map(([id]) => [id, { origin: 'imported' as const, updatedAt: Date.now() }]))
    };
    localStorage.setItem('doraemon-translations', JSON.stringify(translations));
    localStorage.setItem('doraemon-translation-meta', JSON.stringify(translationMeta));
    exportStatus = `Applied ${entries.length} translations by record ID.`;
  }

  async function loadTranslationFile(file: Blob, name: string) {
    loadError = '';
    try {
      importTranslationDocument(JSON.parse(await file.text()));
    } catch (error) {
      loadError = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  async function importTranslationArchive(file: Blob, name: string) {
    loadError = '';
    exportStatus = '';
    try {
      if (!records.length || !archiveBytes) throw new Error('Load the original strings.dat before importing a translated .dat file.');
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

  async function loadDroppedFiles(files: FileList | File[]) {
    const selected = Array.from(files);
    const archive = selected.find((candidate) => candidate.name.toLowerCase().endsWith('.dat'));
    const translation = selected.find((candidate) => candidate.name.toLowerCase().endsWith('.json'));
    if (archive) await importTranslationArchive(archive, archive.name);
    if (translation) await loadTranslationFile(translation, translation.name);
  }

  async function originalFileInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) {
      translations = {};
      translationMeta = {};
      localStorage.removeItem('doraemon-translations');
      localStorage.removeItem('doraemon-translation-meta');
      await loadArchive(input.files[0], input.files[0].name);
    }
    input.value = '';
  }

  async function translatedArchiveInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) await importTranslationArchive(input.files[0], input.files[0].name);
    input.value = '';
  }

  async function translationInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files?.[0]) await loadTranslationFile(input.files[0], input.files[0].name);
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
      lines.forEach((text, line) => { if (text.trim()) segments.push({ id: record.id, line, text }); });
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
    const response = await fetch('/api/translate', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ model: selectedModel, target: targetLanguage, texts: [text] })
    });
    const payload = await response.json();
    if (!response.ok) throw new Error(payload?.error || `Translation server returned HTTP ${response.status}.`);
    const translated = payload?.translations?.[0];
    if (typeof translated !== 'string') throw new Error('Translation server returned no translation text.');
    return translated;
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
      while (queuedRecordIds.length && !generationPaused && !stopRequested) {
        const [id, ...rest] = queuedRecordIds;
        queuedRecordIds = rest;
        const record = recordById(id);
        if (!record) continue;
        activeRecordId = record.id;
        await translateOneRecord(record);
        queueDone += 1;
        translationProgress = queueGoal ? Math.round(queueDone / queueGoal * 100) : 0;
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

  async function translateRecords(sourceRecords: StringRecord[], options: { skipCompleted?: boolean; front?: boolean; force?: boolean } = {}) {
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

  function drop(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    if (event.dataTransfer?.files.length) loadDroppedFiles(event.dataTransfer.files);
  }

  function updateTranslation(id: string, event: Event) {
    saveTranslation(id, (event.currentTarget as HTMLTextAreaElement).value);
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
    window.setTimeout(() => { if (copied === id) copied = ''; }, 1200);
  }

  function copyAll() {
    const text = visibleRecords.map((record) => `${record.id}\t${sourceText(record).replaceAll('\n', '\\N')}`).join('\n');
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
    const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2) + '\n'], { type: 'application/json' }));
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
    const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2) + '\n'], { type: 'application/json' }));
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
      if (!archiveBytes) throw new Error('Load the original strings.dat first.');
      const rebuilt = rebuildStrings(archiveBytes, records, translations);
      const url = URL.createObjectURL(new Blob([rebuilt], { type: 'application/octet-stream' }));
      const link = document.createElement('a');
      link.href = url;
      link.download = 'strings-exported.dat';
      link.click();
      window.setTimeout(() => URL.revokeObjectURL(url), 1000);
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

<svelte:window ondragover={(event) => { event.preventDefault(); dragging = true; }} ondragleave={(event) => { if (!event.relatedTarget) dragging = false; }} ondrop={drop} />

<main>
  <header class="app-header">
    <div>
      <p class="eyebrow">Doraemon Monopoly</p>
      <h1>String translator</h1>
      <p class="subtle">Decode the original text into selectable Traditional Chinese. Font files remain unchanged.</p>
    </div>
    <div class="header-actions">
      <a class="load-button" href="/assets" data-route>Asset viewer</a>
      <label class="load-button">Load original .dat<input type="file" accept=".dat,application/octet-stream" onchange={originalFileInput} /></label>
      <label class="load-button">Import translated .dat<input type="file" accept=".dat,application/octet-stream" onchange={translatedArchiveInput} /></label>
    </div>
  </header>

  <section aria-label="translation import drop area" class:dragging class="drop-zone" ondragenter={(event) => { event.preventDefault(); dragging = true; }} ondragover={(event) => event.preventDefault()} ondrop={drop}>
    <strong>{dragging ? 'Drop translated strings.dat here' : 'Drag translated strings.dat here to import it'}</strong>
    <span>{records.length ? 'The loaded original stays untouched. Matching Chinese records are left blank; changed records fill the translation fields.' : 'First load the original strings.dat above. Dropped .dat files are always treated as translations.'}</span>
  </section>

  {#if loadError}<p class="error" role="alert">{loadError}</p>{/if}

  {#if records.length}
    <section class="summary" aria-label="Translation status">
      <div><span>Source</span><strong>{sourceName}</strong></div>
      <div><span>Records</span><strong>{records.length}</strong></div>
      <div><span>Glyph mapping</span><strong>{missingGlyphs.length ? `${usedGlyphs.size - missingGlyphs.length}/${usedGlyphs.size}` : `${usedGlyphs.size}/${usedGlyphs.size} complete`}</strong></div>
      <div><span>Translated</span><strong>{translatedCount}/{records.length}</strong></div>
    </section>

    {#if missingGlyphs.length}<p class="error">Unmapped glyph IDs: {missingGlyphs.join(', ')}</p>{/if}
    {#if exportStatus}<p class="success" role="status">{exportStatus}</p>{/if}

    <section class="toolbar">
      <div class="fields">
        <label>Search<input type="search" placeholder="ID, Chinese, or translation" bind:value={search} /></label>
        <label>Translate to<select bind:value={targetLanguage}>{#each TARGET_LANGUAGES as language}<option value={language.code}>{language.label}</option>{/each}</select></label>
        <label>Model<select bind:value={selectedModel}>{#each MODELS as model}<option value={model.id}>{model.label}</option>{/each}</select></label>
        <label>From<input class="record-range" placeholder="000/000" bind:value={generateFrom} /></label>
        <label>To<input class="record-range" placeholder="008/000" bind:value={generateTo} /></label>
      </div>
      <div class="actions">
        <button type="button" data-testid="translate-all" disabled={translationRunning || !records.length} onclick={startGenerating}>{translationRunning ? `${translationProgress}% generated` : `Start generating ${selectedTarget.label}`}</button>
        {#if generationPaused && queuedRecordIds.length && !translationRunning}
          <button type="button" data-testid="resume-generation" onclick={resumeGeneration}>Resume</button>
        {/if}
        {#if translationRunning}
          <button type="button" class="quiet" data-testid="pause-generation" disabled={generationPaused || stopRequested} onclick={requestPause}>Pause</button>
        {/if}
        {#if translationRunning || generationPaused}
          <button type="button" class="quiet danger" data-testid="stop-generation" disabled={stopRequested} onclick={requestStop}>Stop</button>
        {/if}
        <button type="button" onclick={copyAll}>{copied === 'all' ? 'Copied' : 'Copy visible TSV'}</button>
        <button type="button" data-testid="export-chinese" onclick={exportChineseRecords}>Export Chinese records</button>
        <button type="button" onclick={exportTranslations}>Export project JSON</button>
        <button type="button" data-testid="export-dat" class="primary" onclick={exportStringsDat}>Export strings.dat</button>
        <button type="button" class="quiet" disabled={!translatedCount} onclick={clearTranslations}>Clear</button>
      </div>
    </section>

    <div class="workspace">
      <aside class="workspace-sidebar">
        <GroupNavigator bind:group {availableGroupIds} onNavigate={selectGroup} />
        <FindReplace bind:find={replaceFind} bind:replacement={replaceWith} matches={replacementMatches} onShow={showReplacement} onReplaceOne={replaceOne} onReplaceAll={replaceAll} />
      </aside>

      <div class="workspace-content">
        {#if translationRunning || translationStage}
          <section class="translation-progress" aria-live="polite">
            <progress max="100" value={translationProgress}></progress>
            <span>{translationStage}</span>
          </section>
        {/if}

        <div class="result-count">{visibleRecords.length} records, {remainingVisibleCount} keep/generatable in current view · {translatedCount} translated · {manualCount} manually edited</div>
        <section class="translation-list" aria-label="Decoded strings">
          {#each visibleRecords as record (record.id)}
            <article id={replacementRecordId(record)} class:queued={queuedRecordSet.has(record.id)} class:translating={activeRecordId === record.id} class:done={!!translations[record.id]?.trim()} class:manual={translationOrigin(record.id) === 'manual'} class:generated={translationOrigin(record.id) === 'generated'} class:imported={translationOrigin(record.id) === 'imported'}>
              <div class="record-heading">
                <code>{record.id}</code>
                <div class="record-actions">
                  {#if generationState(record)}<span class="record-state">{generationState(record)}</span>{/if}
                  {#if translations[record.id]?.trim()}<button type="button" class="copy" onclick={() => regenerateRecord(record)}>Regenerate</button>{/if}
                  <button type="button" class="copy" disabled={!translations[record.id]?.trim() || isLockedForQueue(record.id)} popovertarget={`reflow-${record.id.replace('/', '-')}`}>Reflow…</button>
                  <button type="button" class="copy" onclick={() => copyText(sourceText(record), record.id)}>{copied === record.id ? 'Copied' : 'Copy source'}</button>
                </div>
              </div>
              <div id={`reflow-${record.id.replace('/', '-')}`} class="reflow-popover" popover>
                <strong>Reflow {record.id}</strong>
                <p>Uppercase sysfont advances are used for measuring only; the text’s capitalization is preserved.</p>
                <label>Maximum width (px)<input min="1" max="999" type="number" bind:value={layoutWidth} /></label>
                <div class="reflow-popover-actions">
                  <button type="button" class="quiet" onclick={() => { layoutWidth = GADGETS_LAYOUT.maxWidth; }}>Gadgets preset · 91px</button>
                  <button type="button" class="primary" onclick={() => reflowTranslation(record)}>Reflow text</button>
                </div>
              </div>
              <pre class="source-text" lang="zh-Hant">{sourceText(record)}</pre>
              <label>
                Translation
                <textarea rows={Math.max(2, sourceText(record).split('\n').length)} disabled={isLockedForQueue(record.id)} placeholder="Enter translation…" value={translations[record.id] || ''} oninput={(event) => updateTranslation(record.id, event)}></textarea>
              </label>
            </article>
          {/each}
        </section>
      </div>
    </div>
  {/if}
</main>

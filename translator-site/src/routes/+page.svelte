<script>
  import catalogue from '$lib/catalogue';
  import { onMount } from 'svelte';
  import {
    DUBBING_FORMAT,
    DUBBING_OWNERS,
    compareRecordIds,
    decodeVoiceRecord,
    normalizeAudioFile,
    ownerForVoiceId,
    parseStrings,
    parseVoiceArchive,
    readStoredZip,
    storedZip
  } from '@dubbing-core';
  import { clearLocalFiles, readLocalFile, saveLocalFile } from '$lib/local-store';

  const data = catalogue;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const localKey = 'doraemon-monopoly-translator/v1';

  let language = $state('english');
  let owner = $state('doraemon');
  let query = $state('');
  let edits = $state({});
  let voices = $state({});
  let strings = $state();
  let voice = $state();
  let stringsHash = $state('');
  let voiceHash = $state('');
  let status = $state('Load your original game files to begin. They remain on this device.');
  let error = $state('');

  const sourceById = $derived(
    strings
      ? new Map(parseStrings(strings).map((record) => [record.id, textFromTokens(record.tokens)]))
      : new Map()
  );

  const records = $derived(
    strings
      ? data.languages[language].records
          .filter((record) => record.owner === owner)
          .filter((record) => matchesSearch(record))
          .sort((left, right) => compareRecordIds(left.id, right.id))
      : []
  );

  function textFromTokens(tokens) {
    return tokens
      .map((token) => {
        if (token.type === 'ascii' || token.type === 'vietnamese') return token.text;
        return token.type === 'newline' ? '\n' : '';
      })
      .join('');
  }

  function matchesSearch(record) {
    const value = query.trim().toLowerCase();
    return !value || `${record.id} ${sourceText(record)}`.toLowerCase().includes(value);
  }

  function sourceText(record) {
    return sourceById.get(record.id) ?? '';
  }

  function currentTranslation(record) {
    return edits[record.id]?.translation ?? record.translation ?? '';
  }

  async function sha256(bytes) {
    const digest = await crypto.subtle.digest('SHA-256', bytes.slice().buffer);
    return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
  }

  function supports(kind, hash) {
    const fingerprints = data.fingerprints?.[kind] ?? [];
    return !fingerprints.length || fingerprints.includes(hash);
  }

  async function loadGameFile(kind, file) {
    if (!file) return;

    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const hash = await sha256(bytes);
      if (!supports(kind, hash)) throw new Error(`This ${kind}.dat is not a supported original game file.`);

      if (kind === 'strings') {
        parseStrings(bytes);
        strings = bytes;
        stringsHash = hash;
      } else {
        voice = parseVoiceArchive(bytes);
        voiceHash = hash;
      }

      await saveLocalFile(kind, bytes);
      status = `Loaded ${file.name}. It is saved only in this browser.`;
      error = '';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    }
  }

  function updateTranslation(id, translation) {
    const next = { ...edits };
    const record = data.languages[language].records.find((candidate) => candidate.id === id);
    if (!translation.trim() || translation === record?.translation) delete next[id];
    else next[id] = { ...next[id], translation };
    edits = next;
  }

  async function attachVoice(record, input) {
    const file = input.files?.[0];
    input.value = '';
    if (!file || !record.voiceId) return;

    try {
      voices = { ...voices, [record.voiceId]: await normalizeAudioFile(file) };
      status = `Prepared replacement audio for ${record.id}.`;
      error = '';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    }
  }

  function download(entries, filename) {
    const link = document.createElement('a');
    link.href = URL.createObjectURL(storedZip(entries));
    link.download = filename;
    link.click();
    URL.revokeObjectURL(link.href);
  }

  function saveContribution() {
    const byOwner = new Map();
    for (const [id, edit] of Object.entries(edits)) {
      const record = data.languages[language].records.find((candidate) => candidate.id === id);
      if (!record) continue;
      const translations = byOwner.get(record.owner) ?? [];
      translations.push({ id, ...edit });
      byOwner.set(record.owner, translations);
    }

    const entries = [
      {
        name: `dubbing/${language}/manifest.json`,
        bytes: jsonBytes({
          format: DUBBING_FORMAT,
          language,
          stringsSha256: stringsHash || undefined,
          voiceSha256: voiceHash || undefined
        })
      },
      ...DUBBING_OWNERS.map((currentOwner) => ({
        name: `dubbing/${language}/dialogue/${currentOwner}.json`,
        bytes: jsonBytes({
          format: DUBBING_FORMAT,
          owner: currentOwner,
          records: (byOwner.get(currentOwner) ?? []).sort((left, right) =>
            compareRecordIds(left.id, right.id)
          )
        })
      })),
      ...Object.entries(voices).map(([id, bytes]) => ({
        name: `dubbing/${language}/voices/${ownerForVoiceId(id) ?? 'others'}/${id.replaceAll('/', '-')}.wav`,
        bytes
      }))
    ];

    download(entries, `doraemon-monopoly-${language}-dubbing.zip`);
    status = 'Downloaded contribution ZIP. Attach it to a GitHub Issue.';
  }

  function saveWork() {
    const work = {
      format: 'doraemon-monopoly-work/v1',
      language,
      owner,
      stringsSha256: stringsHash || undefined,
      voiceSha256: voiceHash || undefined,
      edits
    };
    const entries = [
      { name: 'work.json', bytes: jsonBytes(work) },
      ...Object.entries(voices).map(([id, bytes]) => ({
        name: `voices/${id.replaceAll('/', '-')}.wav`,
        bytes
      }))
    ];
    download(entries, 'dubbing-work.zip');
    status = 'Saved your private work ZIP.';
  }

  async function loadWork(input) {
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;

    try {
      const entries = readStoredZip(new Uint8Array(await file.arrayBuffer()));
      const workEntry = entries.find((entry) => entry.name === 'work.json');
      if (!workEntry) throw new Error('This is not a translator work ZIP.');

      const work = JSON.parse(decoder.decode(workEntry.bytes));
      if (work.format !== 'doraemon-monopoly-work/v1') throw new Error('Unsupported work ZIP.');
      if (stringsHash && work.stringsSha256 && stringsHash !== work.stringsSha256)
        throw new Error('This work ZIP belongs to a different strings.dat.');
      if (voiceHash && work.voiceSha256 && voiceHash !== work.voiceSha256)
        throw new Error('This work ZIP belongs to a different voice.dat.');

      language = work.language;
      owner = work.owner;
      edits = work.edits ?? {};
      voices = Object.fromEntries(
        entries.flatMap((entry) => {
          const match = /^voices\/(\d{3})-(\d{3})-(\d{3})\.wav$/i.exec(entry.name);
          return match ? [[`${match[1]}/${match[2]}/${match[3]}`, entry.bytes]] : [];
        })
      );
      status =
        strings || voice ? 'Restored your work ZIP.' : 'Restored work. Load your original files for preview.';
      error = '';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    }
  }

  function playOriginal(voiceId) {
    if (!voice) return;
    const record = voice.records.find((candidate) => candidate.id === voiceId);
    if (!record) return;
    const wav = decodeVoiceRecord(voice, record);
    if (!wav) return;

    const audio = new Audio(URL.createObjectURL(new Blob([wav], { type: 'audio/wav' })));
    audio.onended = () => URL.revokeObjectURL(audio.src);
    void audio.play();
  }

  async function forgetDevice() {
    await clearLocalFiles();
    localStorage.removeItem(localKey);
    strings = undefined;
    voice = undefined;
    stringsHash = '';
    voiceHash = '';
    edits = {};
    voices = {};
    status = 'Forgot local game files and drafts.';
    error = '';
  }

  function jsonBytes(value) {
    return encoder.encode(`${JSON.stringify(value, null, 2)}\n`);
  }

  $effect(() => {
    localStorage.setItem(localKey, JSON.stringify({ language, owner, query, edits }));
  });

  onMount(async () => {
    const saved = localStorage.getItem(localKey);
    if (saved) {
      const state = JSON.parse(saved);
      language = state.language ?? language;
      owner = state.owner ?? owner;
      query = state.query ?? '';
      edits = state.edits ?? {};
    }

    for (const kind of ['strings', 'voice']) {
      const bytes = await readLocalFile(kind);
      if (!bytes) continue;
      const hash = await sha256(bytes);
      if (kind === 'strings') {
        strings = bytes;
        stringsHash = hash;
      } else {
        voice = parseVoiceArchive(bytes);
        voiceHash = hash;
      }
    }

    if (strings || voice) status = 'Restored local game files from this browser.';
  });
</script>

<svelte:head>
  <title>Doraemon Monopoly Translator</title>
  <meta name="description" content="Translate dialogue and voices locally in your browser." />
</svelte:head>

<main class="min-h-screen bg-slate-100 text-slate-900">
  <header class="border-b border-blue-900 bg-blue-950 text-blue-50">
    <div class="mx-auto max-w-6xl px-5 py-10 sm:px-8">
      <p class="text-xs font-bold uppercase tracking-[0.22em] text-cyan-300">Doraemon Monopoly</p>
      <h1 class="mt-3 text-3xl font-bold tracking-tight sm:text-4xl">Translation workspace</h1>
      <p class="mt-3 max-w-2xl text-base leading-7 text-blue-200">
        Translate using your own game files. Nothing is uploaded: your originals and drafts stay in this
        browser.
      </p>
    </div>
  </header>

  <div class="mx-auto max-w-6xl space-y-5 px-5 py-6 sm:px-8 sm:py-8">
    <section class="rounded-2xl bg-white p-5 shadow-sm ring-1 ring-slate-200">
      <div class="flex flex-wrap items-baseline justify-between gap-2">
        <div>
          <p class="text-sm font-bold text-blue-800">Before you translate</p>
          <p class="mt-1 text-sm text-slate-600">Load the original files from your own game installation.</p>
        </div>
        <p class="text-xs font-medium text-slate-500">
          {strings ? 'strings.dat loaded' : 'strings.dat required'} · {voice
            ? 'voice.dat loaded'
            : 'voice.dat optional'}
        </p>
      </div>
      <div class="mt-4 grid gap-3 md:grid-cols-2">
        <label
          class="cursor-pointer rounded-xl border border-dashed border-blue-300 bg-blue-50 p-4 transition hover:border-blue-500"
        >
          <span class="block font-semibold">1. Original strings.dat</span>
          <span class="mt-1 block text-sm text-slate-600">Required for exact text preview.</span>
          <input
            class="mt-3 block w-full text-sm"
            type="file"
            accept=".dat"
            onchange={(event) => loadGameFile('strings', event.currentTarget.files?.[0])}
          />
        </label>
        <label
          class="cursor-pointer rounded-xl border border-dashed border-slate-300 bg-slate-50 p-4 transition hover:border-blue-500"
        >
          <span class="block font-semibold"
            >2. Original voice.dat <span class="font-normal text-slate-500">(optional)</span></span
          >
          <span class="mt-1 block text-sm text-slate-600">Enables original voice playback.</span>
          <input
            class="mt-3 block w-full text-sm"
            type="file"
            accept=".dat"
            onchange={(event) => loadGameFile('voice', event.currentTarget.files?.[0])}
          />
        </label>
      </div>
    </section>

    <section class="rounded-2xl bg-white p-5 shadow-sm ring-1 ring-slate-200">
      <div class="grid gap-4 lg:grid-cols-[1fr_1fr_2fr_auto] lg:items-end">
        <label class="grid gap-1 text-sm font-semibold"
          >Language
          <select class="rounded-lg border border-slate-300 bg-white p-2.5" bind:value={language}>
            <option value="english">English</option>
            <option value="vietnamese">Vietnamese</option>
          </select>
        </label>
        <label class="grid gap-1 text-sm font-semibold"
          >Character
          <select class="rounded-lg border border-slate-300 bg-white p-2.5" bind:value={owner}>
            {#each DUBBING_OWNERS as currentOwner (currentOwner)}<option value={currentOwner}
                >{currentOwner}</option
              >{/each}
          </select>
        </label>
        <label class="grid gap-1 text-sm font-semibold"
          >Find dialogue
          <input
            class="rounded-lg border border-slate-300 p-2.5"
            bind:value={query}
            placeholder="ID or original text"
          />
        </label>
        <p class="text-sm text-slate-500">{records.length} lines shown</p>
      </div>

      <div class="mt-5 flex flex-wrap gap-2 border-t border-slate-200 pt-5">
        <label
          class="cursor-pointer rounded-lg bg-slate-100 px-3 py-2 text-sm font-semibold hover:bg-slate-200"
          >Load work ZIP<input
            class="hidden"
            type="file"
            accept=".zip"
            onchange={(event) => loadWork(event.currentTarget)}
          /></label
        >
        <button
          class="rounded-lg bg-slate-100 px-3 py-2 text-sm font-semibold hover:bg-slate-200"
          onclick={saveWork}>Save work ZIP</button
        >
        <button
          class="rounded-lg bg-cyan-600 px-3 py-2 text-sm font-semibold text-white hover:bg-cyan-700 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={!strings}
          onclick={saveContribution}>Download contribution ZIP</button
        >
        <button
          class="ml-auto rounded-lg px-3 py-2 text-sm font-semibold text-red-700 hover:bg-red-50"
          onclick={forgetDevice}>Forget this device</button
        >
      </div>
    </section>

    {#if error}<p class="rounded-xl border border-red-200 bg-red-50 p-4 text-sm font-medium text-red-800">
        {error}
      </p>{/if}
    <p class="text-sm text-slate-600" aria-live="polite">{status}</p>
    {#if !strings}<p class="rounded-xl border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">
        Load your original <strong>strings.dat</strong> to preview the exact game text before translating.
      </p>{/if}

    <section class="space-y-4" aria-label="Dialogue records">
      {#each records as record (record.id)}
        <article class="rounded-2xl bg-white p-5 shadow-sm ring-1 ring-slate-200">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <strong class="font-mono text-sm text-blue-900">{record.id}</strong>
            <span class="text-xs font-medium text-slate-500"
              >Suggested: {record.maxLines} lines · {record.maxWidth ?? '—'} px</span
            >
          </div>
          <p class="mt-3 whitespace-pre-wrap rounded-lg bg-slate-50 p-3 text-sm leading-6 text-slate-700">
            {sourceText(record)}
          </p>
          <label class="mt-4 block text-sm font-semibold"
            >Your translation
            <textarea
              class="mt-2 min-h-28 w-full rounded-lg border border-slate-300 p-3 font-normal leading-6 outline-none ring-cyan-500 focus:ring-2"
              aria-label={`Translation for ${record.id}`}
              value={currentTranslation(record)}
              oninput={(event) => updateTranslation(record.id, event.currentTarget.value)}
              placeholder="Write the translation"
            ></textarea>
          </label>
          {#if currentTranslation(record).split('\n').length > record.maxLines}<p
              class="mt-2 text-sm font-medium text-amber-700"
            >
              This exceeds the suggested line count.
            </p>{/if}
          {#if record.voiceId}
            <div class="mt-4 flex flex-wrap gap-2">
              <button
                class="rounded-lg bg-slate-100 px-3 py-2 text-sm font-semibold hover:bg-slate-200 disabled:cursor-not-allowed disabled:opacity-50"
                disabled={!voice}
                onclick={() => playOriginal(record.voiceId)}>Play original voice</button
              >
              <label
                class="cursor-pointer rounded-lg bg-slate-100 px-3 py-2 text-sm font-semibold hover:bg-slate-200"
                >{voices[record.voiceId] ? 'Replace voice' : 'Attach replacement voice'}<input
                  class="hidden"
                  type="file"
                  accept="audio/*,.wav,.mp3,.flac,.ogg,.opus,.m4a,.aac"
                  onchange={(event) => attachVoice(record, event.currentTarget)}
                /></label
              >
            </div>
          {/if}
        </article>
      {/each}
    </section>
  </div>
</main>

<script>
  import catalogue from '$lib/catalogue';
  import DialogueRecord from '$lib/DialogueRecord.svelte';
  import GadgetVoiceRecord from '$lib/GadgetVoiceRecord.svelte';
  import VoiceOnlyRecord from '$lib/VoiceOnlyRecord.svelte';
  import ProgressSidebar from '$lib/ProgressSidebar.svelte';
  import SectionNavigator from '$lib/SectionNavigator.svelte';
  import PopoverSelect from '$lib/PopoverSelect.svelte';
  import { gadgetAsset, ownerIcons, ownerLabels, ownerSmallIcons } from '$lib/game-assets';
  import { clearLocalFiles, readLocalFile, saveLocalFile } from '$lib/local-store';
  import { onMount } from 'svelte';
  import {
    DUBBING_FORMAT,
    DUBBING_OWNERS,
    CHIFONT_MAP,
    DIALOG_LAYOUT,
    compareRecordIds,
    decodeVoiceRecord,
    gadgetVoiceId,
    normalizeAudioFile,
    ownerForVoiceId,
    parseStrings,
    parseSysFont,
    parseVoiceArchive,
    reflowGameText,
    readStoredZip,
    storedZip
  } from '@dubbing-core';

  const data = catalogue;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const localKey = 'doraemon-monopoly-translator/v1';

  let language = $state('english');
  let owner = $state('doraemon');
  let selectedSection = $state('dialogue');
  let query = $state('');
  let edits = $state({});
  let voices = $state({});
  let strings = $state();
  let hasStrings = $state(false);
  let voice = $state();
  let sysfontWidths = $state();
  let stringsHash = $state('');
  let voiceHash = $state('');
  let status = $state('Load your original game files to begin. They remain on this device.');
  let error = $state('');
  let showVietnameseNotice = $state(true);

  const sourceById = $derived(
    strings
      ? new Map(parseStrings(strings).map((record) => [record.id, textFromTokens(record.tokens)]))
      : new Map()
  );
  const characterIndex = $derived(DUBBING_OWNERS.slice(0, 6).indexOf(owner));
  const records = $derived(
    strings
      ? data.languages[language].records
          .filter((record) => record.owner === owner)
          .filter((record) => !isIgnoredRecord(record))
          .filter((record) => matchesSearch(record))
          .sort((left, right) => compareRecordIds(left.id, right.id))
      : []
  );
  const gadgetRecords = $derived(
    strings
      ? data.languages[language].records
          .filter((record) => record.gadgetVoiceSlot !== undefined)
          .filter((record) => matchesSearch(record))
          .sort((left, right) => compareRecordIds(left.id, right.id))
      : []
  );
  const voiceOnlyRecords = $derived(
    voice
      ? voice.records.filter(
          (record) =>
            record.path[0] === characterIndex &&
            record.path[1] > 0 &&
            record.path[1] !== 2 &&
            record.path[1] !== 3 &&
            record.storage !== 'empty'
        )
      : []
  );
  const visibleRecordCount = $derived(records.length + gadgetRecords.length + voiceOnlyRecords.length);
  const languageRecords = $derived(data.languages[language].records);
  const dubbedVoiceIds = $derived(
    new Set([...(data.languages[language].dubbedVoiceIds ?? []), ...Object.keys(voices)])
  );
  const characterProgress = $derived(
    DUBBING_OWNERS.slice(0, 6).map((character) => {
      const textRecords = languageRecords.filter(
        (record) => record.owner === character && !isIgnoredRecord(record)
      );
      const textDone = textRecords.filter((record) => currentTranslation(record).trim()).length;
      const voiceIds = (data.languages[language].voiceIds ?? []).filter(
        (id) => ownerForVoiceId(id) === character
      );
      const voiceDone = voiceIds.filter((id) => dubbedVoiceIds.has(id)).length;
      return {
        character,
        textDone,
        textTotal: textRecords.length,
        textPercent: textRecords.length ? Math.round((textDone / textRecords.length) * 100) : 0,
        voiceDone,
        voiceTotal: voiceIds.length,
        voicePercent: voiceIds.length ? Math.round((voiceDone / voiceIds.length) * 100) : 0
      };
    })
  );
  const sectionOptions = $derived([
    { id: 'dialogue', label: `Dialogue · ${records.length}` },
    { id: 'gadgets', label: `Gadget voices · ${gadgetRecords.length}` },
    ...(voiceOnlyRecords.length ? [{ id: 'voices', label: `Other voices · ${voiceOnlyRecords.length}` }] : [])
  ]);
  const sectionIndex = $derived(
    Math.max(
      0,
      sectionOptions.findIndex((section) => section.id === selectedSection)
    )
  );

  function textFromTokens(tokens) {
    return tokens
      .map((token) => {
        if (token.type === 'glyph') return CHIFONT_MAP[token.id] || `⟦g${token.id}⟧`;
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

  function isIgnoredRecord(record) {
    return /^\d{3}\/(019|020|021|022|023|024|026|101|103|125|126|129|131|132)$/.test(record.id);
  }

  function firstLine(text) {
    return text?.split(/\r?\n/, 1)[0]?.trim() ?? '';
  }

  function currentTranslation(record) {
    return edits[record.id]?.translation ?? record.translation ?? '';
  }

  function editableDialogueEdits(input) {
    const editableIds = new Set(
      data.languages[language].records
        .filter((record) => DUBBING_OWNERS.slice(0, 6).includes(record.owner))
        .map((record) => record.id)
    );
    return Object.fromEntries(Object.entries(input ?? {}).filter(([id]) => editableIds.has(id)));
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
        hasStrings = true;
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

  async function loadSysfont(file) {
    if (!file) return;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const font = parseSysFont(bytes);
      sysfontWidths = font.glyphs.slice(0, 128).map((glyph) => glyph.width);
      await saveLocalFile('sysfont', bytes);
      status = `Loaded ${file.name}. Reflow is ready.`;
      error = '';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    }
  }

  function dragOver(event) {
    event.preventDefault();
  }

  async function dropFiles(event) {
    event.preventDefault();
    const files = [...(event.dataTransfer?.files ?? [])];
    const stringsFile = files.find((file) => file.name.toLowerCase() === 'strings.dat');
    const voiceFile = files.find((file) => file.name.toLowerCase() === 'voice.dat');
    const sysfontFile = files.find((file) => file.name.toLowerCase() === 'sysfont.dat');
    if (!stringsFile && !voiceFile && !sysfontFile) {
      error = 'Drop strings.dat, voice.dat, or sysfont.dat.';
      return;
    }
    if (stringsFile) await loadGameFile('strings', stringsFile);
    if (voiceFile) await loadGameFile('voice', voiceFile);
    if (sysfontFile) await loadSysfont(sysfontFile);
  }

  function updateTranslation(id, translation) {
    const next = { ...edits };
    const record = data.languages[language].records.find((candidate) => candidate.id === id);
    if (!translation.trim() || translation === record?.translation) delete next[id];
    else next[id] = { ...next[id], translation };
    edits = next;
  }

  function resetTranslation(id) {
    const next = { ...edits };
    delete next[id];
    edits = next;
  }

  function removeVoice(voiceId) {
    const next = { ...voices };
    delete next[voiceId];
    voices = next;
    void saveVoiceDraft(next);
  }

  async function saveVoiceDraft(value = voices) {
    const plain = Object.fromEntries(
      Object.entries(value).map(([id, bytes]) => [id, new Uint8Array(Array.from(bytes))])
    );
    await saveLocalFile(`draft-voices-${language}`, plain);
  }

  function reflowTranslation(record) {
    const translation = currentTranslation(record);
    if (!translation.trim()) return;
    if (!sysfontWidths) {
      error = 'Load your original sysfont.dat to use game-font reflow.';
      return;
    }
    const preset = DIALOG_LAYOUT;
    const result = reflowGameText(translation, preset.maxWidth, sysfontWidths, preset.splitWords ?? false);
    updateTranslation(record.id, result.text);
    status = result.oversizedWords.length
      ? `Reflowed ${record.id}. Some words are wider than the game box: ${[...new Set(result.oversizedWords)].join(', ')}.`
      : `Reflowed ${record.id} using the ${preset.label} game-font preset.`;
    error = '';
  }

  function voiceOnlyDetails(record) {
    const [character, bank, slot] = record.path;
    const id = `${String(character).padStart(3, '0')}/${String(bank).padStart(3, '0')}/${String(slot).padStart(3, '0')}`;
    if (bank === 1 && slot <= 10) return { title: 'Menu voice', detail: id };
    if (bank === 1 && slot <= 15) return { title: 'Action voice', detail: `${id} · shared game action` };
    if (bank === 1 && slot < 28) return { title: 'Misc voice', detail: id };
    if (bank === 1 && slot < 64)
      return { title: `Alphabet · ${'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'[slot - 28]}`, detail: id };
    return { title: `Additional voice · bank ${bank}`, detail: id };
  }

  function navigateSection(section) {
    selectedSection = section;
    document.getElementById(`section-${section}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  function moveSection(direction) {
    const index = Math.max(
      0,
      sectionOptions.findIndex((section) => section.id === selectedSection)
    );
    const next = sectionOptions[Math.min(sectionOptions.length - 1, Math.max(0, index + direction))];
    navigateSection(next.id);
  }

  async function attachVoice(voiceId, file) {
    if (!file) return;
    try {
      voices = { ...voices, [voiceId]: await normalizeAudioFile(file) };
      await saveVoiceDraft(voices);
      status = `Prepared replacement audio for ${voiceId}.`;
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
      if (!record || !DUBBING_OWNERS.slice(0, 6).includes(record.owner)) continue;
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
    download(
      [
        { name: 'work.json', bytes: jsonBytes(work) },
        ...Object.entries(voices).map(([id, bytes]) => ({
          name: `voices/${id.replaceAll('/', '-')}.wav`,
          bytes
        }))
      ],
      'dubbing-work.zip'
    );
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
      edits = editableDialogueEdits(work.edits);
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
    const record = voice?.records.find((candidate) => candidate.id === voiceId);
    const wav = record ? decodeVoiceRecord(voice, record) : undefined;
    playAudioBytes(wav);
  }

  function playNew(voiceId) {
    playAudioBytes(voices[voiceId]);
  }

  function playAudioBytes(wav) {
    if (!wav) return;
    const audio = new Audio(URL.createObjectURL(new Blob([wav], { type: 'audio/wav' })));
    audio.onended = () => URL.revokeObjectURL(audio.src);
    void audio.play();
  }

  async function forgetDevice() {
    await clearLocalFiles();
    localStorage.removeItem(localKey);
    strings = undefined;
    hasStrings = false;
    voice = undefined;
    sysfontWidths = undefined;
    stringsHash = '';
    voiceHash = '';
    edits = {};
    voices = {};
    await saveVoiceDraft({});
    status = 'Forgot local game files and drafts.';
    error = '';
  }

  function jsonBytes(value) {
    return encoder.encode(`${JSON.stringify(value, null, 2)}\n`);
  }

  $effect(() => {
    localStorage.setItem(localKey, JSON.stringify({ language, owner, selectedSection, query, edits }));
  });

  onMount(async () => {
    const saved = localStorage.getItem(localKey);
    if (saved) {
      const state = JSON.parse(saved);
      language = state.language ?? language;
      owner = DUBBING_OWNERS.slice(0, 6).includes(state.owner) ? state.owner : owner;
      selectedSection = state.selectedSection ?? selectedSection;
      query = state.query ?? '';
      edits = editableDialogueEdits(state.edits);
    }
    for (const kind of ['strings', 'voice', 'sysfont']) {
      const bytes = await readLocalFile(kind);
      if (!bytes) continue;
      const hash = await sha256(bytes);
      if (kind === 'strings') {
        strings = bytes;
        hasStrings = true;
        stringsHash = hash;
      } else if (kind === 'voice') {
        voice = parseVoiceArchive(bytes);
        voiceHash = hash;
      } else {
        const font = parseSysFont(bytes);
        sysfontWidths = font.glyphs.slice(0, 128).map((glyph) => glyph.width);
      }
    }
    const savedVoices = await readLocalFile(`draft-voices-${language}`);
    if (savedVoices && typeof savedVoices === 'object' && !(savedVoices instanceof Uint8Array)) {
      voices = savedVoices;
    }
    if (strings || voice) status = 'Restored original files.';
  });

  onMount(() => {
    const sectionIds = ['dialogue', 'gadgets', 'voices'];

    const updateSection = () => {
      const anchor = window.innerHeight * 0.32;
      const sections = sectionIds
        .map((id) => ({ id, element: document.getElementById(`section-${id}`) }))
        .filter((section) => section.element)
        .sort((left, right) => left.element.offsetTop - right.element.offsetTop);
      if (!sections.length) return;
      const current = sections.reduce((active, section) => {
        const top = section.element.getBoundingClientRect().top;
        return top <= anchor ? section.id : active;
      }, sections[0].id);
      if (current !== selectedSection) selectedSection = current;
    };

    let frame;
    const onScroll = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(updateSection);
    };
    window.addEventListener('scroll', onScroll, { passive: true });
    window.addEventListener('resize', onScroll, { passive: true });
    const mutationObserver = new MutationObserver(onScroll);
    mutationObserver.observe(document.querySelector('main'), { childList: true, subtree: true });
    onScroll();

    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener('scroll', onScroll);
      window.removeEventListener('resize', onScroll);
      mutationObserver.disconnect();
    };
  });
</script>

<svelte:head>
  <title>Doraemon Monopoly Translator</title>
  <meta
    name="description"
    content="Translate Doraemon Monopoly dialogue and voices locally in your browser."
  />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
</svelte:head>

<main class="scrollbar-main min-h-screen overflow-x-clip text-ink" ondragover={dragOver} ondrop={dropFiles}>
  <header
    class="relative border-b-4 border-navy bg-[linear-gradient(115deg,#75ccf7_0%,#b6edff_52%,#76c8f4_100%)]"
  >
    <div
      class="pointer-events-none absolute inset-0 opacity-25 bg-[radial-gradient(circle_at_20px_20px,#fff_0_2px,transparent_3px)] bg-size-[48px_48px]"
    ></div>
    <div
      class="relative mx-auto flex max-w-[1600px] items-center justify-between gap-5 px-5 py-4 sm:px-8 lg:py-5"
    >
      <div class="flex min-w-0 items-center gap-4">
        <img
          class="h-20 w-36 shrink-0 object-contain sm:h-24 sm:w-44"
          src="/game-assets/doraemon-monopoly-logo.png"
          alt="Doraemon Monopoly"
        />
        <div class="min-w-0">
          <h1
            class="text-2xl font-black tracking-tight text-white drop-shadow-[0_3px_0_rgb(7_82_173)] sm:text-4xl"
          >
            Translator workshop
          </h1>
        </div>
      </div>
    </div>
  </header>

  <nav class="border-b border-navy/30 bg-navy text-white" aria-label="Workspace controls">
    <div
      class="mx-auto flex max-w-[1600px] flex-wrap items-center gap-x-5 gap-y-2 px-5 py-3 text-sm font-bold sm:px-8"
    >
      <span class="rounded-full bg-white/15 px-3 py-1"
        >{hasStrings ? '✓ strings.dat ready' : '1 · Load strings.dat'}</span
      >
      <span class="rounded-full bg-white/15 px-3 py-1"
        >{voice ? '✓ voice preview ready' : '2 · voice.dat optional'}</span
      >
      {#if hasStrings}<span class="ml-auto text-white/85">{visibleRecordCount} records in this view</span
        >{/if}
    </div>
  </nav>

  <div class="mx-auto max-w-[1600px] px-5 py-6 sm:px-8 sm:py-8">
    {#if error}
      <p
        class="mb-5 rounded-2xl border-2 border-danger/35 bg-red-50 px-5 py-4 text-sm font-bold text-danger"
        role="alert"
      >
        {error}
      </p>
    {/if}

    {#if !hasStrings}
      <section class="game-panel mx-auto max-w-4xl overflow-hidden">
        <div class="border-b border-outline bg-sky/30 px-6 py-5 sm:px-8">
          <p class="text-xs font-black uppercase tracking-[0.2em] text-navy">Your first stop</p>
          <h2 class="mt-2 text-2xl font-black text-ink sm:text-3xl">Bring your own original game files</h2>
          <p class="mt-2 max-w-2xl text-sm leading-6 text-ink/75">
            The translator reads the text directly in your browser. Drop <code>strings.dat</code>,
            <code>voice.dat</code>, and optionally <code>sysfont.dat</code> anywhere here—or use the buttons. Original
            files never enter a ZIP or leave this device.
          </p>
        </div>
        <div class="grid gap-4 p-6 sm:grid-cols-2 sm:p-8">
          <label class="file-card border-accent-blue bg-sky/20">
            <span class="grid h-11 w-11 place-items-center rounded-xl bg-accent-blue text-xl text-white"
              >Aa</span
            >
            <span class="mt-4 block text-lg font-black text-navy">Load strings.dat</span>
            <span class="mt-1 block text-sm leading-6 text-ink/70"
              >Required. Unlocks original dialogue and the translation workspace.</span
            >
            <input
              class="sr-only"
              type="file"
              accept=".dat"
              onchange={(event) => loadGameFile('strings', event.currentTarget.files?.[0])}
            />
            <span class="mt-5 inline-flex rounded-xl bg-accent-blue px-4 py-2 text-sm font-black text-white"
              >Choose strings.dat</span
            >
          </label>
          <label class="file-card border-outline bg-white">
            <span class="grid h-11 w-11 place-items-center rounded-xl bg-accent-yellow text-xl text-navy"
              >♫</span
            >
            <span class="mt-4 block text-lg font-black text-navy">Load voice.dat</span>
            <span class="mt-1 block text-sm leading-6 text-ink/70"
              >Optional. Lets you listen to and replace original in-game voices.</span
            >
            <input
              class="sr-only"
              type="file"
              accept=".dat"
              onchange={(event) => loadGameFile('voice', event.currentTarget.files?.[0])}
            />
            <span
              class="mt-5 inline-flex rounded-xl border-2 border-outline bg-white px-4 py-2 text-sm font-black text-navy"
              >Choose voice.dat</span
            >
          </label>
        </div>
        <div
          class="flex flex-wrap items-center justify-between gap-3 border-t border-outline bg-ice-panel px-6 py-4 text-sm sm:px-8"
        >
          <p class="text-ink/70">Already have a backup from another computer?</p>
          <label class="cursor-pointer font-black text-accent-blue hover:underline"
            >Load work ZIP<input
              class="hidden"
              type="file"
              accept=".zip"
              onchange={(event) => loadWork(event.currentTarget)}
            /></label
          >
        </div>
      </section>
    {:else}
      <section class="game-panel mb-5 overflow-hidden">
        <div class="flex flex-wrap items-center gap-3 border-b border-outline bg-white/80 px-4 py-3 sm:px-5">
          <div class="w-44 shrink-0 text-sm font-black text-navy">
            <PopoverSelect
              label="Language"
              value={language}
              options={[
                { value: 'english', label: 'English' },
                { value: 'vietnamese', label: 'Vietnamese' }
              ]}
              onChange={(value) => (language = value)}
            />
          </div>
          <label class="min-w-48 flex-1 inline-flex items-center text-sm font-black text-navy"
            ><span class="text-nowrap">Find a line</span>
            <input
              class="ml-2 w-full max-w-sm rounded-xl border-2 border-outline bg-white px-3 py-2 font-medium text-ink outline-none placeholder:text-ink/40 focus:border-accent-blue"
              bind:value={query}
              placeholder="ID or original text"
            />
          </label>
          <div class="ml-auto flex flex-wrap gap-2">
            <label class="action-button cursor-pointer border-outline bg-white text-navy"
              >Re-strings<input
                class="hidden"
                type="file"
                accept=".dat"
                onchange={(event) => loadGameFile('strings', event.currentTarget.files?.[0])}
              /></label
            >
            <label class="action-button cursor-pointer border-outline bg-white text-navy"
              >{voice ? 'Re-voice' : 'Ld-voice'}<input
                class="hidden"
                type="file"
                accept=".dat"
                onchange={(event) => loadGameFile('voice', event.currentTarget.files?.[0])}
              /></label
            >
            <label class="action-button cursor-pointer border-outline bg-white text-navy"
              >{sysfontWidths ? 'Re-sysfont' : 'Ld-sysfont'}<input
                class="hidden"
                type="file"
                accept=".dat"
                onchange={(event) => loadSysfont(event.currentTarget.files?.[0])}
              /></label
            >
            <label class="action-button cursor-pointer border-outline bg-white text-navy"
              >Load work<input
                class="hidden"
                type="file"
                accept=".zip"
                onchange={(event) => loadWork(event.currentTarget)}
              /></label
            >
            <button class="action-button bg-accent-yellow text-navy" onclick={saveWork}>Save work</button>
            <button class="action-button bg-accent-blue text-white" onclick={saveContribution}>Export</button>
          </div>
        </div>
        <div class="flex gap-2 overflow-x-auto px-4 py-3 sm:px-5" aria-label="Choose character or game group">
          {#each DUBBING_OWNERS.slice(0, 6) as currentOwner (currentOwner)}
            <button
              class:owner-selected={owner === currentOwner}
              class="owner-chip"
              onclick={() => (owner = currentOwner)}
            >
              {#if ownerIcons[currentOwner]}
                <img
                  class="rounded-full border-navy border-2 h-12 w-12"
                  src={ownerIcons[currentOwner]}
                  alt=""
                />
              {:else}
                <span class="grid h-9 w-9 place-items-center rounded-full bg-accent-yellow text-lg">★</span
                >{/if}
              <span>{ownerLabels[currentOwner]}</span>
            </button>
          {/each}
        </div>
      </section>

      <div class="grid gap-5 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <section class="space-y-4 pb-24" aria-label="Dialogue records">
          <div id="section-dialogue" class="scroll-mt-6 pt-1">
            <p class="text-xs font-black uppercase tracking-[0.18em] text-navy">
              {ownerLabels[owner]} · dialogue
            </p>
          </div>
          {#if records.length === 0 && gadgetRecords.length === 0}
            <div class="game-panel px-6 py-10 text-center">
              <p class="text-lg font-black text-navy">No lines match this search.</p>
              <p class="mt-2 text-sm text-ink/65">Try an ID, another character, or clear the filter.</p>
            </div>
          {/if}
          <div class="grid gap-3 [grid-template-columns:repeat(auto-fit,minmax(22rem,1fr))]">
            {#each records as record (record.id)}
              <DialogueRecord
                {record}
                source={sourceText(record)}
                translation={currentTranslation(record)}
                {language}
                edited={!!edits[record.id]}
                voiceEdited={!!(record.voiceId && voices[record.voiceId])}
                voiceId={record.voiceId}
                hasVoice={!!voice}
                translateHref={`https://translate.google.com/?sl=zh-TW&tl=${language === 'vietnamese' ? 'vi' : 'en'}&text=${encodeURIComponent(sourceText(record))}&op=translate`}
                onTranslation={(value) => updateTranslation(record.id, value)}
                onReflow={() => reflowTranslation(record)}
                onPlayOriginal={() => playOriginal(record.voiceId)}
                onPlayNew={() => playNew(record.voiceId)}
                onReplace={(file) => attachVoice(record.voiceId, file)}
                onRemove={() => removeVoice(record.voiceId)}
                onReset={() => resetTranslation(record.id)}
              />
            {/each}
          </div>

          {#if gadgetRecords.length}
            <div id="section-gadgets" class="scroll-mt-6 pt-4">
              <p class="mb-3 text-xs font-black uppercase tracking-[0.18em] text-navy">
                {ownerLabels[owner]} · gadget voices
              </p>
              <p class="mb-4 text-sm text-ink/70">
                Reference titles from the shared gadget descriptions. Only this character's voice is editable
                here.
              </p>
            </div>
          {/if}
          <div class="grid gap-3 [grid-template-columns:repeat(auto-fit,minmax(20rem,1fr))]">
            {#each gadgetRecords as record (record.id)}
              {@const voiceId = gadgetVoiceId(characterIndex, record.gadgetVoiceSlot)}
              <GadgetVoiceRecord
                {record}
                {voiceId}
                ownerLabel={ownerLabels[owner]}
                sourceLine={firstLine(sourceText(record))}
                translationLine={firstLine(record.translation)}
                gadgetSrc={gadgetAsset(record.gadgetAssetId)}
                hasVoice={!!voice}
                edited={!!voices[voiceId]}
                onPlayOriginal={() => playOriginal(voiceId)}
                onPlayNew={() => playNew(voiceId)}
                onReplace={(file) => attachVoice(voiceId, file)}
                onRemove={() => removeVoice(voiceId)}
              />
            {/each}
          </div>

          {#if voiceOnlyRecords.length}
            <section id="section-voices" class="game-panel scroll-mt-6 overflow-hidden">
              <div class="border-b border-outline bg-ice-panel px-5 py-4">
                <p class="text-xs font-black uppercase tracking-[0.18em] text-navy">
                  {ownerLabels[owner]} · voice library
                </p>
                <p class="mt-1 text-sm text-ink/70">Menu, actions, misc, and alphabet recordings.</p>
              </div>
              <div class="p-2 grid gap-2 grid-cols-[repeat(auto-fit,minmax(10rem,1fr))]">
                {#each voiceOnlyRecords as voiceRecord (voiceRecord.id)}
                  {@const details = voiceOnlyDetails(voiceRecord)}
                  <VoiceOnlyRecord
                    title={details.title}
                    detail={details.detail}
                    voiceId={voiceRecord.id}
                    edited={!!voices[voiceRecord.id]}
                    hasVoice={!!voice}
                    onPlayOriginal={() => playOriginal(voiceRecord.id)}
                    onPlayNew={() => playNew(voiceRecord.id)}
                    onReplace={(file) => attachVoice(voiceRecord.id, file)}
                    onRemove={() => removeVoice(voiceRecord.id)}
                  />
                {/each}
              </div>
            </section>
          {/if}
        </section>

        <ProgressSidebar
          progress={characterProgress}
          {ownerLabels}
          {ownerSmallIcons}
          {language}
          {showVietnameseNotice}
          {status}
          onOwner={(value) => (owner = value)}
          onDismissVietnamese={() => (showVietnameseNotice = false)}
          onForget={forgetDevice}
        />
      </div>
      <SectionNavigator
        {sectionOptions}
        {selectedSection}
        {sectionIndex}
        {owner}
        {ownerLabels}
        {ownerIcons}
        onSection={navigateSection}
        onOwner={(value) => (owner = value)}
        onMove={moveSection}
      />
    {/if}
  </div>
</main>

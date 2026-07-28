<script lang="ts">
  let {
    search = $bindable(),
    targetLanguage = $bindable(),
    model = $bindable(),
    from = $bindable(),
    to = $bindable(),
    settingsOpen = $bindable(false),
    replaceOpen = $bindable(false),
    languages,
    models,
    visibleCount,
    translatedCount,
    targetLabel,
    running,
    paused,
    stopping,
    queuedCount,
    progress,
    copiedAll,
    onStart,
    onResume,
    onPause,
    onStop,
    onCopy,
    onClear
  }: {
    search: string;
    targetLanguage: string;
    model: string;
    from: string;
    to: string;
    settingsOpen: boolean;
    replaceOpen: boolean;
    languages: { code: string; label: string }[];
    models: { id: string; label: string }[];
    visibleCount: number;
    translatedCount: number;
    targetLabel: string;
    running: boolean;
    paused: boolean;
    stopping: boolean;
    queuedCount: number;
    progress: number;
    copiedAll: boolean;
    onStart: () => void;
    onResume: () => void;
    onPause: () => void;
    onStop: () => void;
    onCopy: () => void;
    onClear: () => void;
  } = $props();
</script>

<section class="translation-command-bar">
  <div class="command-search">
    <label
      >Find in this group<input
        type="search"
        placeholder="ID, source, or translation"
        bind:value={search}
      /></label
    >
    <span>{visibleCount} records · {translatedCount} translated</span>
  </div>
  <div class="command-actions">
    <button type="button" data-testid="translate-all" disabled={running} onclick={onStart}>
      {running ? `${progress}% generated` : `Start generating ${targetLabel}`}
    </button>
    {#if paused && queuedCount && !running}<button
        type="button"
        data-testid="resume-generation"
        onclick={onResume}>Resume</button
      >{/if}
    {#if running}<button
        type="button"
        class="quiet"
        data-testid="pause-generation"
        disabled={paused || stopping}
        onclick={onPause}>Pause</button
      >{/if}
    {#if running || paused}<button
        type="button"
        class="quiet danger"
        data-testid="stop-generation"
        disabled={stopping}
        onclick={onStop}>Stop</button
      >{/if}
    <button type="button" onclick={onCopy}>{copiedAll ? 'Copied' : 'Copy visible TSV'}</button>
    <button
      type="button"
      class:active={replaceOpen}
      aria-expanded={replaceOpen}
      onclick={() => (replaceOpen = !replaceOpen)}>{replaceOpen ? 'Close replace' : 'Find & replace'}</button
    >
    <button type="button" class:active={settingsOpen} onclick={() => (settingsOpen = !settingsOpen)}
      >Translation settings</button
    >
    <button type="button" class="quiet" disabled={!translatedCount} onclick={onClear}>Clear</button>
  </div>
</section>

{#if settingsOpen}
  <section class="translation-settings" aria-label="Translation settings">
    <label
      >Translate to<select bind:value={targetLanguage}
        >{#each languages as language (language.code)}<option value={language.code}>{language.label}</option
          >{/each}</select
      ></label
    >
    <label
      >Model<select bind:value={model}
        >{#each models as item (item.id)}<option value={item.id}>{item.label}</option>{/each}</select
      ></label
    >
    <label>From record<input class="record-range" placeholder="000/000" bind:value={from} /></label>
    <label>To record<input class="record-range" placeholder="008/000" bind:value={to} /></label>
  </section>
{/if}

<script>
  let {
    mode,
    open,
    coupon,
    cloudEnabled,
    localSavedAt,
    cloudSavedAt,
    busy,
    onClose,
    onLocal,
    onCloud,
    onDownload,
    onFile
  } = $props();

  function formatTime(ts) {
    if (!ts) return 'never';
    return new Date(ts).toLocaleString();
  }

  const localNewer = $derived(localSavedAt && cloudSavedAt ? localSavedAt > cloudSavedAt : false);
  const cloudNewer = $derived(localSavedAt && cloudSavedAt ? cloudSavedAt > localSavedAt : false);

  function pickFile(event) {
    const input = event.currentTarget;
    onFile?.(input);
    input.value = '';
  }
</script>

{#if open}
  <div
    class="audio-modal-backdrop"
    role="presentation"
    onclick={(event) => event.target === event.currentTarget && onClose()}
  >
    <div class="audio-modal" role="dialog" aria-modal="true">
      <div class="mb-4 flex items-start justify-between gap-3">
        <div>
          <p class="text-xs font-black uppercase tracking-[0.2em] text-navy">
            {mode === 'save' ? 'Save work' : 'Load work'}
          </p>
          <h2 class="mt-1 text-xl font-black text-ink">
            {mode === 'save' ? 'Back up your session' : 'Restore a saved session'}
          </h2>
        </div>
        <button class="action-button shrink-0" onclick={onClose} aria-label="Close" disabled={busy !== null}
          >✕</button
        >
      </div>

      <div class="space-y-3">
        {#if mode === 'load'}
          <label
            class="flex cursor-pointer items-center gap-3 rounded-2xl border-2 border-outline bg-white p-4 transition hover:border-accent-blue disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span
              class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-accent-yellow text-lg text-navy"
              >📁</span
            >
            <span class="flex-1">
              <span class="block text-sm font-black text-navy">Open a ZIP file</span>
              <span class="block text-xs text-ink/60">From another computer or an earlier download.</span>
            </span>
            <input class="hidden" type="file" accept=".zip" onchange={pickFile} />
            <span class="text-sm font-black text-navy">Choose ›</span>
          </label>
        {/if}

        <button
          class="flex w-full items-center gap-3 rounded-2xl border-2 border-outline bg-white p-4 text-left transition hover:border-accent-blue disabled:cursor-not-allowed disabled:opacity-50"
          onclick={onLocal}
          disabled={busy !== null}
        >
          <span class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-sky/30 text-lg text-navy"
            >⌂</span
          >
          <span class="flex-1">
            <span class="block text-sm font-black text-navy">This browser</span>
            <span class="block text-xs text-ink/60"
              >{busy === 'local'
                ? mode === 'save'
                  ? 'Saving…'
                  : 'Loading…'
                : `Last saved ${formatTime(localSavedAt)}`}</span
            >
          </span>
          {#if localNewer}
            <span
              class="rounded-full bg-accent-yellow px-2 py-0.5 text-[10px] font-black uppercase tracking-wide text-navy"
              >newer</span
            >
          {/if}
          <span class="text-sm font-black text-navy">{mode === 'save' ? 'Save' : 'Load'} ›</span>
        </button>

        <div>
          <button
            class="flex w-full items-center gap-3 rounded-2xl border-2 border-outline bg-white p-4 text-left transition hover:border-accent-blue disabled:cursor-not-allowed disabled:opacity-50"
            onclick={onCloud}
            disabled={busy !== null || !cloudEnabled || !coupon.trim()}
          >
            <span
              class="grid h-10 w-10 shrink-0 place-items-center rounded-xl bg-accent-blue text-lg text-white"
              >☁</span
            >
            <span class="flex-1">
              <span class="block text-sm font-black text-navy">Cloud</span>
              <span class="block text-xs text-ink/60"
                >{busy === 'cloud'
                  ? mode === 'save'
                    ? 'Saving…'
                    : 'Loading…'
                  : cloudEnabled
                    ? `Last saved ${formatTime(cloudSavedAt)}`
                    : 'Unavailable in this build'}</span
              >
            </span>
            {#if cloudNewer}
              <span
                class="rounded-full bg-accent-yellow px-2 py-0.5 text-[10px] font-black uppercase tracking-wide text-navy"
                >newer</span
              >
            {/if}
            <span class="text-sm font-black text-navy">{mode === 'save' ? 'Save' : 'Load'} ›</span>
          </button>
          {#if cloudEnabled}
            {#if !coupon.trim()}
              <p class="mt-2 text-xs text-ink/60">
                Enter your project coupon in the Project coupon box at the top to enable cloud storage.
              </p>
            {/if}
          {/if}
        </div>

        {#if mode === 'save'}
          <button
            class="flex w-full items-center justify-center gap-2 rounded-2xl border-2 border-outline bg-ice-panel px-4 py-3 text-sm font-black text-navy transition hover:bg-sky/30 disabled:cursor-not-allowed disabled:opacity-50"
            onclick={onDownload}
            disabled={busy !== null}>Download ZIP</button
          >
        {/if}
      </div>
    </div>
  </div>
{/if}

<script>
  let {
    progress,
    ownerLabels,
    ownerSmallIcons,
    language,
    showVietnameseNotice,
    status,
    onOwner,
    onDismissVietnamese,
    onForget
  } = $props();
</script>

<aside class="space-y-4 lg:sticky lg:top-5 lg:self-start">
  <section class="game-panel p-5">
    <p class="text-xs font-black uppercase tracking-[0.18em] text-navy">Device storage</p>
    <p class="mt-2 text-sm leading-6 text-ink/70">{status}</p>
    <button class="mt-4 text-sm font-black text-danger hover:underline" onclick={onForget}>Forget</button>
  </section>
  <section class="game-panel p-5">
    <p class="text-xs font-black uppercase tracking-[0.18em] text-navy">Character progress</p>
    <div class="mt-4 space-y-4">
      {#each progress as item (item.character)}
        <button
          class="block w-full rounded-xl p-2 text-left hover:bg-sky/15"
          onclick={() => onOwner(item.character)}
        >
          <div class="flex items-center gap-3">
            <img class="h-8 w-8 object-contain" src={ownerSmallIcons[item.character]} alt="" />
            <div class="min-w-0 flex-1">
              <div class="flex items-center justify-between gap-2 text-sm font-black text-navy">
                <span>{ownerLabels[item.character]}</span><span class="text-xs text-ink/60"
                  >{item.textPercent}% text</span
                >
              </div>
              <div class="mt-1 h-2 overflow-hidden rounded-full bg-sky/35">
                <div class="h-full rounded-full bg-accent-blue" style={`width: ${item.textPercent}%`}></div>
              </div>
              <div class="mt-2 flex items-center justify-between gap-2 text-xs font-bold text-ink/60">
                <span>{item.textDone}/{item.textTotal} translated</span><span>{item.voicePercent}% voice</span
                >
              </div>
              <div class="mt-1 h-2 overflow-hidden rounded-full bg-sky/35">
                <div class="h-full rounded-full bg-success" style={`width: ${item.voicePercent}%`}></div>
              </div>
            </div>
          </div>
        </button>
      {/each}
    </div>
  </section>
  {#if language === 'vietnamese' && showVietnameseNotice}
    <section class="game-panel border-warning bg-accent-yellow p-5">
      <p class="text-xs font-black uppercase tracking-[0.18em] text-warning">Vietnamese custom font</p>
      <p class="mt-2 text-sm leading-6 text-warning">
        Reflow uses the supplied game font and may not match Vietnamese line breaks perfectly.
      </p>
      <button class="mt-3 text-sm font-black text-warning hover:underline" onclick={onDismissVietnamese}
        >Dismiss</button
      >
    </section>
  {/if}
</aside>

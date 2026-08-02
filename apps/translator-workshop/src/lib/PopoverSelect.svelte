<script>
  let { label, value, options, onChange, dropup = false, compact = false } = $props();
  let open = $state(false);
  let root;

  function closeOutside(event) {
    if (open && root && !root.contains(event.target)) open = false;
  }
</script>

<svelte:window onclick={closeOutside} />
<div class="relative" bind:this={root}>
  <button
    class="flex w-full items-center justify-between gap-4 rounded-xl border-2 border-outline bg-white px-3 py-2 text-left text-sm font-black text-ink shadow-sm hover:border-accent-blue"
    aria-haspopup="listbox"
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    {#if compact && options.find((option) => option.value === value)?.icon}
      <img
        class="h-8 w-8 object-contain"
        src={options.find((option) => option.value === value).icon}
        alt={label}
      />
    {:else}
      <span>{label ? `${label}: ` : ''}{options.find((option) => option.value === value)?.label ?? value}</span>
    {/if}
    <span class:chevron-open={open} class="popover-chevron" aria-hidden="true">⌄</span>
  </button>
  {#if open}
    <div
      class={`absolute left-0 z-10 space-y-2 max-h-72 min-w-full overflow-auto rounded-xl border-2 border-outline bg-white p-1 shadow-[0_14px_35px_rgb(7_82_173/0.24)] scrollbar-popover ${dropup ? 'popover-menu-up' : 'popover-menu-down'}`}
      style={dropup
        ? 'bottom: 100%; top: auto; margin-bottom: 0.5rem; margin-top: 0;'
        : 'top: 100%; bottom: auto; margin-top: 0.5rem; margin-bottom: 0;'}
      role="listbox"
    >
      {#each options as option (option.value)}
        <button
          class={`block w-full rounded-lg px-3 py-2 text-left text-sm font-bold text-navy ${option.value === value ? 'bg-accent-yellow' : 'hover:bg-accent-blue/20'}`}
          role="option"
          aria-selected={option.value === value}
          onclick={() => {
            onChange(option.value);
            open = false;
          }}
          >{#if compact && option.icon}<img
              class="mx-auto h-9 w-9 object-contain"
              src={option.icon}
              alt={option.label}
            />{:else}{option.label}{/if}</button
        >
      {/each}
    </div>
  {/if}
</div>

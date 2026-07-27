<script>
  import PopoverSelect from '$lib/PopoverSelect.svelte';

  let {
    sectionOptions,
    selectedSection,
    sectionIndex,
    owner,
    ownerLabels,
    ownerIcons,
    onSection,
    onOwner,
    onMove
  } = $props();
</script>

<nav
  class="fixed bottom-4 left-1/2 z-50 flex w-[min(94vw,36rem)] -translate-x-1/2 flex-wrap items-center gap-2 rounded-2xl border-2 border-outline bg-white/95 p-2 shadow-[0_12px_35px_rgb(7_82_173/0.25)] backdrop-blur"
  aria-label="Record sections"
>
  <div class="min-w-0 flex-1 basis-full sm:basis-auto">
    <PopoverSelect
      label="Grouping"
      value={selectedSection}
      options={sectionOptions.map(({ id, label }) => ({ value: id, label }))}
      dropup
      onChange={onSection}
    />
  </div>
  <div class="w-22 min-w-0">
    <PopoverSelect
      label="Character"
      value={owner}
      options={Object.keys(ownerLabels).map((character) => ({
        value: character,
        label: ownerLabels[character],
        icon: ownerIcons[character]
      }))}
      compact
      dropup
      onChange={onOwner}
    />
  </div>
  <button
    class="action-button border-outline bg-white text-navy disabled:opacity-35"
    disabled={sectionIndex === 0}
    title="Previous group"
    aria-label="Previous group"
    onclick={() => onMove(-1)}>◀</button
  >
  <button
    class="action-button border-outline bg-white text-navy disabled:opacity-35"
    disabled={sectionIndex === sectionOptions.length - 1}
    title="Next group"
    aria-label="Next group"
    onclick={() => onMove(1)}>▶</button
  >
</nav>

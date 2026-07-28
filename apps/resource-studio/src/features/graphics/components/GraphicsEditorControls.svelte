<script lang="ts">
  let {
    selection = $bindable(),
    dragging = $bindable(false),
    busy,
    hasPalette,
    modifiedCount,
    archiveLabel,
    onExportPng,
    onDropPng,
    onImportPng,
    onExportArchive
  }: {
    selection: string;
    dragging: boolean;
    busy: boolean;
    hasPalette: boolean;
    modifiedCount: number;
    archiveLabel: string;
    onExportPng: () => void;
    onDropPng: (event: DragEvent) => void;
    onImportPng: (event: Event) => void;
    onExportArchive: () => void;
  } = $props();
</script>

<section class="sprite-editor" aria-label="Indexed graphics editor">
  <div class="sprite-export">
    <strong>Export indexed PNGs</strong>
    <label>IDs<input type="text" inputmode="numeric" placeholder="1-10, 15" bind:value={selection} /></label>
    <button type="button" disabled={busy || !hasPalette} onclick={onExportPng}>Export PNG ZIP</button>
  </div>
  <div
    class:dragging
    class="sprite-import"
    role="group"
    aria-label="Indexed PNG import"
    ondragover={(event) => {
      event.preventDefault();
      dragging = true;
    }}
    ondragleave={() => (dragging = false)}
    ondrop={onDropPng}
  >
    <strong>Import indexed PNG replacements</strong>
    <label class="file-button"
      >Choose PNGs<input type="file" accept="image/png,.png" multiple onchange={onImportPng} /></label
    >
  </div>
  <div class="sprite-save">
    <span><b>{modifiedCount}</b> modified</span>
    <button type="button" class="primary" disabled={busy || !modifiedCount} onclick={onExportArchive}
      >Export modified {archiveLabel}</button
    >
  </div>
</section>

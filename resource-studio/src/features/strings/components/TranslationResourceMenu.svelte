<script lang="ts">
  let {
    hasRecords,
    hasArchive,
    hasVoice,
    onOriginalStrings,
    onModifiedStrings,
    onSysfont,
    onOriginalVoice,
    onModifiedVoice,
    onExportSource,
    onExportProject,
    onExportStrings,
    onExportVoice,
    dubbingAvailable = false,
    dubbingStatus = '',
    onSyncDubbing = () => undefined,
    onSaveDubbing = () => undefined,
    onCheckDubbing = () => undefined
  }: {
    hasRecords: boolean;
    hasArchive: boolean;
    hasVoice: boolean;
    onOriginalStrings: (event: Event) => void;
    onModifiedStrings: (event: Event) => void;
    onSysfont: (event: Event) => void;
    onOriginalVoice: (event: Event) => void;
    onModifiedVoice: (event: Event) => void;
    onExportSource: () => void;
    onExportProject: () => void;
    onExportStrings: () => void;
    onExportVoice: () => void;
    dubbingAvailable?: boolean;
    dubbingStatus?: string;
    onSyncDubbing?: () => void;
    onSaveDubbing?: () => void;
    onCheckDubbing?: () => void;
  } = $props();
</script>

<section class="resource-actions" aria-label="Resource files and exports">
  <details class="resource-menu">
    <summary>Files &amp; exports</summary>
    <div class="resource-menu-panel">
      <label class="load-button"
        >Original strings.dat<input
          type="file"
          accept=".dat,application/octet-stream"
          onchange={onOriginalStrings}
        />
      </label>
      <label class="load-button"
        >Modified strings.dat<input
          type="file"
          accept=".dat,application/octet-stream"
          onchange={onModifiedStrings}
        />
      </label>
      <label class="load-button"
        >sysfont.dat<input type="file" accept=".dat,application/octet-stream" onchange={onSysfont} />
      </label>
      <label class="load-button"
        >Original voice.dat<input
          type="file"
          accept=".dat,application/octet-stream"
          onchange={onOriginalVoice}
        />
      </label>
      <label class="load-button"
        >Modified voice.dat<input
          type="file"
          accept=".dat,application/octet-stream"
          onchange={onModifiedVoice}
        />
      </label>
      <button type="button" data-testid="export-chinese" disabled={!hasRecords} onclick={onExportSource}
        >Export source JSON</button
      >
      <button type="button" disabled={!hasRecords} onclick={onExportProject}>Export project JSON</button>
      <button
        type="button"
        data-testid="export-dat"
        class="primary"
        disabled={!hasArchive}
        onclick={onExportStrings}>Export strings.dat</button
      >
      <button type="button" class="primary" disabled={!hasVoice} onclick={onExportVoice}
        >Export voice.dat</button
      >
      {#if dubbingAvailable}
        <hr />
        <button type="button" disabled={!hasRecords} onclick={onSyncDubbing}>Sync from dubbing</button>
        <button type="button" disabled={!hasRecords} onclick={onSaveDubbing}>Save to dubbing</button>
        <button type="button" onclick={onCheckDubbing}>Check dubbing</button>
        {#if dubbingStatus}<small>{dubbingStatus}</small>{/if}
      {/if}
    </div>
  </details>
</section>

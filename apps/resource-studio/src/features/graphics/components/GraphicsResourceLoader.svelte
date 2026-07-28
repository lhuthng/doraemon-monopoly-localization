<script lang="ts">
  let {
    onDrop,
    onInput
  }: {
    onDrop: (event: DragEvent) => void;
    onInput: (event: Event, kind?: 'sprite1' | 'sprite2') => void;
  } = $props();
</script>

<section
  class="load-panel"
  aria-label="Asset loaders"
  ondragover={(event) => event.preventDefault()}
  ondrop={onDrop}
>
  <strong>Game archives</strong>
  <div>
    <label class="file-button"
      >Load bitmaps.dat<input
        type="file"
        accept=".dat,application/octet-stream"
        onchange={(event) => onInput(event)}
      /></label
    >
    <label class="file-button"
      >Load Sprite1.dat<input
        type="file"
        accept=".dat,application/octet-stream"
        onchange={(event) => onInput(event, 'sprite1')}
      /></label
    >
    <label class="file-button"
      >Load sprite2.dat<input
        type="file"
        accept=".dat,application/octet-stream"
        onchange={(event) => onInput(event, 'sprite2')}
      /></label
    >
  </div>
</section>

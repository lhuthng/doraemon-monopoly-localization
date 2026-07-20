<script lang="ts">
  export type ReplaceMatch = { id: string; start: number };

  let {
    find = $bindable(''),
    replacement = $bindable(''),
    matches = [],
    onShow,
    onReplaceOne,
    onReplaceAll
  }: {
    find?: string;
    replacement?: string;
    matches?: ReplaceMatch[];
    onShow: (find: string, index: number) => void;
    onReplaceOne: (find: string, replacement: string, index: number) => void;
    onReplaceAll: (find: string, replacement: string) => void;
  } = $props();

  let index = $state(-1);
  let status = $state('');

  function reset() {
    index = -1;
    status = '';
  }

  function move(direction: number) {
    if (!find) {
      status = 'Enter text to find first.';
      return;
    }
    if (!matches.length) {
      status = 'No editable translated text matches this search.';
      return;
    }
    index =
      index < 0
        ? direction > 0
          ? 0
          : matches.length - 1
        : (index + direction + matches.length) % matches.length;
    onShow(find, index);
    status = `Match ${index + 1} of ${matches.length}: ${matches[index].id}`;
  }

  function replaceOne() {
    if (!find) {
      status = 'Enter text to find first.';
      return;
    }
    if (!matches.length) {
      status = 'No editable translated text matches this search.';
      return;
    }
    index = index < 0 ? 0 : Math.min(index, matches.length - 1);
    const before = matches.length;
    onReplaceOne(find, replacement, index);
    status = `Replaced one match. ${before - 1} remaining matches will be recalculated.`;
    index = -1;
  }

  function replaceAll() {
    if (!find) {
      status = 'Enter text to find first.';
      return;
    }
    const count = matches.length;
    onReplaceAll(find, replacement);
    status = count ? `Replaced ${count} matches.` : 'No editable translated text matches this search.';
    index = -1;
  }
</script>

<section class="side-card replace-card" aria-label="Replace translated text">
  <div class="side-card-heading">
    <span>Find & replace</span>
    <strong>Translations only</strong>
  </div>
  <label>
    Find
    <textarea
      rows="3"
      placeholder="Text or multiple lines to find"
      bind:value={find}
      oninput={reset}
      spellcheck="false"></textarea>
  </label>
  <label>
    Replace with
    <textarea
      rows="3"
      placeholder="Replacement text; press Enter to insert a newline"
      bind:value={replacement}
      spellcheck="false"></textarea>
  </label>
  <div class="replace-navigation">
    <button type="button" class="quiet" disabled={!find} onclick={() => move(-1)}>↑ Previous</button>
    <button type="button" class="quiet" disabled={!find} onclick={() => move(1)}>Next ↓</button>
  </div>
  <div class="side-button-pair">
    <button type="button" disabled={!find} onclick={replaceOne}>Replace</button>
    <button type="button" class="primary" disabled={!find} onclick={replaceAll}>Replace all</button>
  </div>
  <p class="side-status" aria-live="polite">
    {status ||
      (find
        ? `${matches.length} editable match${matches.length === 1 ? '' : 'es'}`
        : 'Enter text to search translated fields.')}
  </p>
</section>

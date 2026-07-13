<script lang="ts">
  import { STRING_GROUPS } from '../groups';

  let {
    group = $bindable('all'),
    availableGroupIds = [],
    onNavigate
  }: {
    group?: string;
    availableGroupIds?: string[];
    onNavigate: (group: string) => void;
  } = $props();

  let groups = $derived(STRING_GROUPS.filter((item) => availableGroupIds.includes(item.id)));
  let selected = $derived(groups.find((item) => item.id === group));

  function select(id: string) {
    group = id;
    onNavigate(id);
  }

  function move(direction: number) {
    if (!groups.length) return;
    const current = groups.findIndex((item) => item.id === group);
    const index = current < 0 ? (direction > 0 ? 0 : groups.length - 1) : (current + direction + groups.length) % groups.length;
    select(groups[index].id);
  }
</script>

<section class="side-card group-navigator" aria-label="Group navigation">
  <div class="side-card-heading">
    <span>Browse</span>
    <strong>{selected ? `${selected.id} · ${selected.label}` : 'All groups'}</strong>
  </div>
  <label>
    String group
    <select value={group} onchange={(event) => select((event.currentTarget as HTMLSelectElement).value)}>
      <option value="all">All groups</option>
      {#each groups as item}
        <option value={item.id}>{item.id} — {item.label}</option>
      {/each}
    </select>
  </label>
  {#if selected}<p>{selected.detail}</p>{/if}
  <div class="side-button-pair">
    <button type="button" class="quiet" onclick={() => move(-1)}>← Previous</button>
    <button type="button" class="quiet" onclick={() => move(1)}>Next →</button>
  </div>
</section>

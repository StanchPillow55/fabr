<script lang="ts">
  import Group from '../ui/Group.svelte';
  import Row from '../ui/Row.svelte';
  import { COLUMNS, STATE_LABEL, folderName, folderOf, groupByFolder, groupByState, stepLabel, type Entry } from './board';

  let { entries, onopen }: { entries: Entry[]; onopen: (e: Entry) => void } = $props();

  let folder = $state('all');
  const folders = $derived([...groupByFolder(entries).keys()]);
  const shown = $derived(folder === 'all' ? entries : entries.filter((e) => folderOf(e.path) === folder));
  const cols = $derived(groupByState(shown));
</script>

<div class="board">
  <label class="board__filter">Folder
    <select bind:value={folder}>
      <option value="all">all</option>
      {#each folders as f (f)}<option value={f} title={f}>{folderName(f)}</option>{/each}
    </select>
  </label>
  {#each COLUMNS as c (c.id)}
    <Group header="{c.label} ({cols[c.id].length})">
      {#each cols[c.id] as e (e.path)}
        {@const r = e.record}
        {@const step = stepLabel(r)}
        <Row
          label={r.title}
          sub={[folderName(folderOf(e.path)), r.due ? `due ${r.due}` : '', step ? `step: ${step}` : ''].filter(Boolean).join(' · ')}
          value={[c.id === 'done' ? STATE_LABEL[r.state] : '', r.waiting ? 'waiting' : ''].filter(Boolean).join(' · ')}
          tone={r.waiting ? 'tint' : undefined}
          onclick={() => onopen(e)}
        />
      {:else}
        <p class="board__empty">Nothing here</p>
      {/each}
    </Group>
  {/each}
</div>

<style>
  .board { display: flex; flex-direction: column; gap: 14px; }
  .board__filter { display: flex; align-items: center; gap: 8px; font-size: var(--fs-12); color: var(--label-2); }
  .board__empty { margin: 0; padding: 10px 16px; color: var(--label-3); font-style: italic; }
</style>

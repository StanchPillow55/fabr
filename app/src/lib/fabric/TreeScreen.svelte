<script lang="ts">
  import Group from '../ui/Group.svelte';
  import Row from '../ui/Row.svelte';
  import { COLUMNS, STATE_LABEL, countsFor, folderName, groupByFolder, stepLabel, type Entry } from './board';

  let { entries, onopen }: { entries: Entry[]; onopen: (e: Entry) => void } = $props();

  let selected = $state<string | null>(null);
  const folders = $derived(groupByFolder(entries));
  const list = $derived(selected ? (folders.get(selected) ?? []) : []);
</script>

{#if selected}
  <button type="button" class="tree__up" onclick={() => (selected = null)}>All folders</button>
  <Group header={folderName(selected)}>
    {#each list as e (e.path)}
      {@const r = e.record}
      {@const step = stepLabel(r)}
      <Row
        label={r.title}
        sub={[r.due ? `due ${r.due}` : '', step ? `step: ${step}` : ''].filter(Boolean).join(' · ')}
        value={[STATE_LABEL[r.state], r.waiting ? 'waiting' : ''].filter(Boolean).join(' · ')}
        onclick={() => onopen(e)}
      />
    {/each}
  </Group>
{:else}
  <Group header="Folders">
    {#each [...folders] as [name, items] (name)}
      {@const c = countsFor(items)}
      <Row label={folderName(name)} sub={name !== folderName(name) ? name : undefined} onclick={() => (selected = name)}>
        <span class="tree__counts">
          {#each COLUMNS as col (col.id)}
            <span class:tree__zero={c.byColumn[col.id] === 0} title={col.label}>{c.byColumn[col.id]}</span>
          {/each}
          <span class="tree__wait" class:tree__zero={c.waiting === 0} title="waiting">{c.waiting} waiting</span>
        </span>
      </Row>
    {:else}
      <p class="tree__empty">No folders hold records</p>
    {/each}
  </Group>
  <p class="tree__legend">Counts: reminder, task, defined, in progress, succeeded or failed, then waiting.</p>
{/if}

<style>
  .tree__counts { display: flex; gap: 8px; align-items: baseline; font-variant-numeric: tabular-nums; }
  .tree__zero { color: var(--label-3); }
  .tree__wait { color: var(--orange); font-weight: 600; font-size: var(--fs-12); }
  .tree__wait.tree__zero { color: var(--label-3); font-weight: 400; }
  .tree__up { background: none; border: 0; color: var(--tint); padding: 4px 0; cursor: pointer; align-self: flex-start; }
  .tree__empty { margin: 0; padding: 10px 16px; color: var(--label-3); font-style: italic; }
  .tree__legend { margin: 6px 4px 0; font-size: var(--fs-12); color: var(--label-3); }
</style>

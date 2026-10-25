<script lang="ts">
  import { COLUMNS } from './folder';
  import StageBar from './StageBar.svelte';
  import { mostAdvanced, statusOf } from './stage';
  import type { FabricStore } from './tasks.svelte';

  let { fabric, folder }: { fabric: FabricStore; folder: string | null } = $props();

  const c = $derived(fabric.countsFor(folder));
  const active = $derived(fabric.agentsFor(folder).filter((v) => v.activity !== 'parked'));
  const lead = $derived(mostAdvanced(active));
  const shown = $derived(COLUMNS.filter((col) => c.byColumn[col.id] > 0));
</script>

{#if c.total > 0}
  <span class="tstrip" data-testid="task-strip" title="{c.total} {c.total === 1 ? 'task' : 'tasks'}">
    {#each shown as col (col.id)}<span class="tstrip__n" title={col.label}>{c.byColumn[col.id]}<span class="tstrip__k">{col.id === 'in_progress' ? 'run' : col.id === 'done' ? 'done' : col.id}</span></span>{/each}
    {#if c.waiting > 0}<span class="tstrip__wait" title="Waiting for you">waiting</span>{/if}
    {#if lead}
      <span class="tstrip__bar" title="Agent {statusOf(lead, fabric.now)}: {lead.stage}"><StageBar view={lead} now={fabric.now} /></span>
      {#if active.length > 1}<span class="tstrip__more">+{active.length - 1}</span>{/if}
    {/if}
  </span>
{/if}

<style>
  .tstrip { display: inline-flex; align-items: baseline; gap: 6px; flex: none; font-size: var(--fs-12); color: var(--label-2); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .tstrip__k { margin-left: 2px; color: var(--label-3); }
  .tstrip__wait { color: var(--orange); font-weight: 600; }
  .tstrip__bar { flex: none; width: 72px; align-self: center; }
  .tstrip__more { color: var(--label-3); }
</style>

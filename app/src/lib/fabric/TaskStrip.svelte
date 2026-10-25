<script lang="ts">
  import { COLUMNS } from './folder';
  import type { FabricStore } from './tasks.svelte';

  let { fabric, folder }: { fabric: FabricStore; folder: string | null } = $props();

  const c = $derived(fabric.countsFor(folder));
  const running = $derived(fabric.runningFor(folder));
  const shown = $derived(COLUMNS.filter((col) => c.byColumn[col.id] > 0));
</script>

{#if c.total > 0}
  <span class="tstrip" data-testid="task-strip" title="{c.total} {c.total === 1 ? 'task' : 'tasks'}">
    {#each shown as col (col.id)}<span class="tstrip__n" title={col.label}>{c.byColumn[col.id]}<span class="tstrip__k">{col.id === 'in_progress' ? 'run' : col.id === 'done' ? 'done' : col.id}</span></span>{/each}
    {#if c.waiting > 0}<span class="tstrip__wait" title="Waiting for you">waiting</span>{/if}
    {#if running}<span class="tstrip__run" title="Agent running: {running.step}">running</span>{/if}
  </span>
{/if}

<style>
  .tstrip { display: inline-flex; align-items: baseline; gap: 6px; flex: none; font-size: var(--fs-12); color: var(--label-2); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .tstrip__k { margin-left: 2px; color: var(--label-3); }
  .tstrip__wait { color: var(--orange); font-weight: 600; }
  .tstrip__run { color: var(--green); font-weight: 600; }
</style>

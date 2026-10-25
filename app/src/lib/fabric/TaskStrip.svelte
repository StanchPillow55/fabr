<script lang="ts">
  import StageBar from './StageBar.svelte';
  import { MAX_BARS } from '../constants';
  import { barAgents } from '../model';
  import { PHASE_LABEL } from './phase';
  import { statusOf } from './stage';
  import type { FabricStore } from './tasks.svelte';

  let { fabric, folder, part = 'counts' }: { fabric: FabricStore; folder: string | null; part?: 'counts' | 'bars' } = $props();

  const c = $derived(fabric.countsFor(folder));
  const agents = $derived(barAgents(fabric.agentsFor(folder)));
  const bars = $derived(agents.slice(0, MAX_BARS));
  const shown = $derived(
    [
      { id: 'todo', label: 'Todo (reminders)', n: c.todo },
      { id: 'active', label: 'Working, needs you, needs harness or paused', n: c.active },
      { id: 'suspended', label: 'Suspended', n: c.suspended },
      { id: 'done', label: 'Done', n: c.done },
      { id: 'failed', label: 'Failed', n: c.failed },
    ].filter((x) => x.n > 0),
  );
</script>

{#if part === 'bars'}
  {#each bars as v (v.path)}
    <span class="tstrip__bar" title="Agent {statusOf(v, fabric.now)}: {v.stage}"><StageBar view={v} now={fabric.now} /></span>
  {/each}
  {#if agents.length > MAX_BARS}<span class="tstrip__more">+{agents.length - MAX_BARS}</span>{/if}
{:else if c.total > 0}
  <span class="tstrip" data-testid="task-strip" title="{c.total} {c.total === 1 ? 'task' : 'tasks'}">
    {#each shown as col (col.id)}<span class="tstrip__n" title={col.label}>{col.n}<span class="tstrip__k">{col.id}</span></span>{/each}
    {#if c.needs_you > 0}<span class="tstrip__wait" title="Waiting for you">{PHASE_LABEL.needs_you}</span>{/if}
    {#if c.needs_harness > 0}<span class="tstrip__harness" title="Needs a machine with the harness">{PHASE_LABEL.needs_harness}</span>{/if}
  </span>
{/if}

<style>
  .tstrip { display: inline-flex; align-items: baseline; gap: 6px; min-width: 0; overflow: hidden; font-size: var(--fs-12); color: var(--label-2); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .tstrip__k { margin-left: 2px; color: var(--label-3); }
  .tstrip__wait { color: var(--orange); font-weight: 600; }
  .tstrip__harness { color: var(--orange); font-weight: 600; }
  .tstrip__bar { flex: none; width: 56px; align-self: center; }
  .tstrip__more { color: var(--label-3); }
</style>

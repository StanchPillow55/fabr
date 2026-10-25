<script lang="ts">
  import { onMount } from 'svelte';
  import Group from '../ui/Group.svelte';
  import Icon from '../ui/Icon.svelte';
  import Row from '../ui/Row.svelte';
  import { isOpen, PHASE_LABEL, type Phase } from './phase';
  import { suspendLabel } from './stage';
  import { stepLabel } from './folder';
  import TaskSheet from './TaskSheet.svelte';
  import type { FabricStore, Held } from './tasks.svelte';

  let { fabric, folder }: { fabric: FabricStore; folder: string } = $props();

  const list = $derived(fabric.tasksFor(folder));
  const of = (p: Phase) => list.filter((h) => fabric.phaseFor(h) === p);
  const counts = $derived(fabric.countsFor(folder));
  const SHOWN: Phase[] = ['needs_you', 'working', 'paused', 'todo'];
  const FOLDED: { phase: Phase; testid: string }[] = [
    { phase: 'suspended', testid: 'task-suspended' },
    { phase: 'done', testid: 'task-done' },
  ];

  onMount(() => () => fabric.open(null));

  async function tick(h: Held) {
    try {
      await fabric.complete(h);
      fabric.actionError = null;
    } catch (e) {
      fabric.actionError = (e as Error).message;
    }
  }
</script>

{#snippet line(h: Held)}
  {@const r = h.record}
  {@const phase = fabric.phaseFor(h)}
  {@const step = stepLabel(r)}
  {@const live = fabric.agents.get(h.path)?.activity}
  <div class="trow" data-testid="task-row" data-state={r.state} data-phase={phase}>
    <Row
      label={r.title}
      sub={[r.due ? `due ${r.due.replace('T', ' ')}` : '', r.pri ? '!'.repeat(r.pri) : '', step ? `step: ${step}` : ''].filter(Boolean).join(' · ') || undefined}
      value={[phase === 'suspended' ? suspendLabel(r).toLowerCase() : PHASE_LABEL[phase], live && live !== 'parked' && phase === 'working' ? 'running' : ''].filter(Boolean).join(' · ')}
      tone={phase === 'needs_you' ? 'tint' : undefined}
      onclick={() => fabric.open(h.path)}
    />
    {#if isOpen(phase) && !r.requests.complete}
      <button type="button" class="btn btn--icon trow__tick" data-testid="mark-done" title="Mark done" aria-label="Mark {r.title} done" onclick={() => tick(h)}><Icon name="check" size={14} /></button>
    {/if}
  </div>
{/snippet}

{#if list.length}
  <section class="nodeset__section" data-testid="task-section">
    <h3 class="section-title">Tasks <span class="section-title__count">{counts.todo} todo · {counts.active} active</span></h3>
    {#each SHOWN as p (p)}
      {@const rows = of(p)}
      {#if rows.length}
        <Group header={PHASE_LABEL[p]}>
          {#each rows as h (h.path)}{@render line(h)}{/each}
        </Group>
      {/if}
    {/each}
    {#each FOLDED as f (f.phase)}
      {@const rows = of(f.phase)}
      {#if rows.length}
        <details class="taskdone" data-testid={f.testid}>
          <summary class="taskdone__summary">{PHASE_LABEL[f.phase]} <span class="section-title__count">{rows.length}</span></summary>
          <Group>
            {#each rows as h (h.path)}{@render line(h)}{/each}
          </Group>
        </details>
      {/if}
    {/each}
    {#if of('failed').length}
      <Group header={PHASE_LABEL.failed}>
        {#each of('failed') as h (h.path)}{@render line(h)}{/each}
      </Group>
    {/if}
    {#if fabric.actionError}<p class="hint">{fabric.actionError}</p>{/if}
  </section>
{/if}

<TaskSheet {fabric} {folder} />

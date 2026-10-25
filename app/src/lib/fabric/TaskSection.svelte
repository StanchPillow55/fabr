<script lang="ts">
  import { onMount } from 'svelte';
  import Group from '../ui/Group.svelte';
  import Icon from '../ui/Icon.svelte';
  import Row from '../ui/Row.svelte';
  import { effectiveSuspended, suspendLabel } from './stage';
  import { canMarkDone } from './done';
  import { STATE_LABEL, stepLabel } from './folder';
  import TaskSheet from './TaskSheet.svelte';
  import type { FabricStore, Held } from './tasks.svelte';

  let { fabric, folder }: { fabric: FabricStore; folder: string } = $props();

  const list = $derived(fabric.tasksFor(folder));
  const finished = $derived(list.filter((h) => h.record.state === 'succeeded' || h.record.state === 'failed'));
  const shelved = $derived(list.filter((h) => !finished.includes(h) && effectiveSuspended(h.record, fabric.now)));
  const live = $derived(list.filter((h) => !finished.includes(h) && !shelved.includes(h)));
  const todo = $derived(live.filter((h) => h.record.state === 'reminder'));
  const active = $derived(live.filter((h) => ['task', 'defined', 'in_progress'].includes(h.record.state)));

  onMount(() => () => fabric.open(null));

  async function tick(h: Held) {
    try {
      await fabric.markDone(h.path);
      fabric.actionError = null;
    } catch (e) {
      fabric.actionError = (e as Error).message;
    }
  }
</script>

{#snippet line(h: Held)}
  {@const r = h.record}
  {@const step = stepLabel(r)}
  {@const live = fabric.agents.get(h.path)?.activity}
  <div class="trow" data-testid="task-row" data-state={r.state}>
    <Row
      label={r.title}
      sub={[r.due ? `due ${r.due.replace('T', ' ')}` : '', r.pri ? '!'.repeat(r.pri) : '', step ? `step: ${step}` : ''].filter(Boolean).join(' · ') || undefined}
      value={[STATE_LABEL[r.state], effectiveSuspended(r, fabric.now) ? suspendLabel(r).toLowerCase() : '', r.paused ? 'paused' : '', r.waiting ? 'waiting' : '', live && live !== 'parked' ? 'running' : ''].filter(Boolean).join(' · ')}
      tone={r.waiting ? 'tint' : undefined}
      onclick={() => fabric.open(h.path)}
    />
    {#if canMarkDone(r)}
      <button type="button" class="btn btn--icon trow__tick" data-testid="mark-done" title="Mark done" aria-label="Mark {r.title} done" onclick={() => tick(h)}><Icon name="check" size={14} /></button>
    {/if}
  </div>
{/snippet}

{#if list.length}
  <section class="nodeset__section" data-testid="task-section">
    <h3 class="section-title">Tasks <span class="section-title__count">{todo.length} todo · {active.length} active</span></h3>
    {#if todo.length || active.length}
      <Group>
        {#each todo as h (h.path)}{@render line(h)}{/each}
        {#each active as h (h.path)}{@render line(h)}{/each}
      </Group>
    {/if}
    {#if shelved.length}
      <details class="taskdone" data-testid="task-suspended">
        <summary class="taskdone__summary">Suspended <span class="section-title__count">{shelved.length}</span></summary>
        <Group>
          {#each shelved as h (h.path)}{@render line(h)}{/each}
        </Group>
      </details>
    {/if}
    {#if finished.length}
      <details class="taskdone" data-testid="task-done">
        <summary class="taskdone__summary">Done <span class="section-title__count">{finished.length}</span></summary>
        <Group>
          {#each finished as h (h.path)}{@render line(h)}{/each}
        </Group>
      </details>
    {/if}
    {#if fabric.actionError}<p class="hint">{fabric.actionError}</p>{/if}
  </section>
{/if}

<TaskSheet {fabric} {folder} />

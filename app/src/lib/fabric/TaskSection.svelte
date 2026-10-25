<script lang="ts">
  import { onMount } from 'svelte';
  import Group from '../ui/Group.svelte';
  import Row from '../ui/Row.svelte';
  import Sheet from '../ui/Sheet.svelte';
  import AgentsScreen from './AgentsScreen.svelte';
  import StageBar from './StageBar.svelte';
  import { finishedView } from './stage';
  import { STATE_LABEL, folderName, folderOf, stepLabel } from './folder';
  import RecordScreen from './RecordScreen.svelte';
  import type { FabricStore } from './tasks.svelte';

  let { fabric, folder }: { fabric: FabricStore; folder: string } = $props();

  const list = $derived(fabric.tasksFor(folder));
  const open = $derived(fabric.openRecord && folderOf(fabric.openRecord.path) === folder ? fabric.openRecord : null);

  onMount(() => () => fabric.open(null));
</script>

{#if list.length}
  <section class="panel__section" data-testid="task-section">
    <h3 class="section-title">Tasks <span class="section-title__count">{list.length}</span></h3>
    <Group>
      {#each list as h (h.path)}
        {@const r = h.record}
        {@const step = stepLabel(r)}
        <Row
          label={r.title}
          sub={[r.due ? `due ${r.due}` : '', step ? `step: ${step}` : ''].filter(Boolean).join(' · ') || undefined}
          value={[STATE_LABEL[r.state], r.waiting ? 'waiting' : '', fabric.agents.get(h.path)?.activity && fabric.agents.get(h.path)?.activity !== 'parked' ? 'running' : ''].filter(Boolean).join(' · ')}
          tone={r.waiting ? 'tint' : undefined}
          onclick={() => fabric.open(h.path)}
        />
      {/each}
    </Group>
    {#if fabric.actionError}<p class="hint">{fabric.actionError}</p>{/if}
  </section>
{/if}

{#if open}
  {@const h = open}
  <Sheet open={true} title={h.record.title} width={720} onclose={() => fabric.open(null)}>
    <RecordScreen
      record={h.record}
      name={folderName(folder)}
      backLabel="Close"
      now={fabric.now}
      onback={() => fabric.open(null)}
      onanswer={(a) => fabric.answer(h, a)}
      onapprove={(d) => fabric.approve(h, d)}
      onrequest={(k) => fabric.request(h, k)}
      onreload={() => fabric.reload(h.path)}
    >
      {#snippet agent()}
        {@const view = fabric.agents.get(h.path)}
        {@const done = finishedView(h)}
        {#if view}
          <AgentsScreen agents={[view]} detail now={fabric.now} oncancel={(p) => fabric.cancelTask(p)} />
        {:else if done}
          <StageBar view={done} mode="full" now={fabric.now} />
        {/if}
      {/snippet}
    </RecordScreen>
  </Sheet>
{/if}

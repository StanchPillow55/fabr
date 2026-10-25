<script lang="ts">
  import Group from '../ui/Group.svelte';
  import NavRow from '../ui/NavRow.svelte';
  import Row from '../ui/Row.svelte';
  import type { SettingsController } from './controller.svelte';

  let { ctl }: { ctl: SettingsController } = $props();
</script>

{#if ctl.drafts.length}
  <Group header="Sources">
    {#each ctl.drafts as d (d.uid)}
      {@const st = ctl.status(d)}
      <NavRow label={d.title} sub={`${d.src.host || 'local'} · ${d.src.root || 'no root'}`} onclick={() => ctl.push({ kind: 'source', uid: d.uid })}>
        {#snippet lead()}<span class="row-dot row-dot--{st}" title={st === 'ok' ? 'Connected' : st === 'error' ? 'Error' : 'Not tested'}></span>{/snippet}
      </NavRow>
    {/each}
  </Group>
{/if}
<Group>
  <Row label="Add source…" tone="tint" onclick={() => ctl.addSource()} />
</Group>

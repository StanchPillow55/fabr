<script lang="ts">
  import type { Item } from '../api/types';
  import Group from '../ui/Group.svelte';
  import Row from '../ui/Row.svelte';

  let { items }: { items: Item[] } = $props();

  const ORDER = ['todo', 'wip', 'done'];
  const LABEL = { todo: 'todo', wip: 'active', done: 'done' } as const;
  const ordered = $derived([...items].sort((a, b) => ORDER.indexOf(a.status) - ORDER.indexOf(b.status)));
</script>

{#if ordered.length}
  <Group>
    {#each ordered as i (i.id)}
      <Row label={i.title} sub={[i.due ? `due ${i.due.replace('T', ' ')}` : '', i.pri ? '!'.repeat(i.pri) : ''].filter(Boolean).join(' · ') || undefined} value={LABEL[i.status]} />
    {/each}
  </Group>
{:else}
  <p class="hint">No tasks.</p>
{/if}

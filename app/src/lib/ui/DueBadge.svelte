<script lang="ts">
  import type { Item } from '../api/types';
  import { dueClass, dueLabel } from '../model';
  let { item, today, full = false }: { item: Item; today: string; full?: boolean } = $props();
  const cls = $derived(dueClass(item, today));
</script>

{#if item.due}<span class="due" class:due--overdue={cls === 'overdue'} class:due--soon={cls === 'soon'} title={item.due.replace('T', ' ')}>{dueLabel(item, today, full)}</span>{/if}

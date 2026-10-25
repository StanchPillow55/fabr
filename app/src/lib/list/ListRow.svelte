<script lang="ts">
  import type { ListRow } from '../model';
  import DueBadge from '../ui/DueBadge.svelte';
  import Pill from '../ui/Pill.svelte';
  import Priority from '../ui/Priority.svelte';

  let { row, onopen, today = '', showSource = false, sourceName = '', selected = false, color = null }: {
    row: ListRow;
    onopen: () => void;
    today?: string;
    showSource?: boolean;
    sourceName?: string;
    selected?: boolean;
    color?: string | null;
  } = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.target === e.currentTarget && e.key === 'Enter') onopen();
  }
</script>

<tr class="table__row row--{row.item.phase}" class:table__row--selected={selected} tabindex="0" onclick={onopen} {onkeydown}>
  {#if showSource}<td class="table__td table__td--muted">{sourceName || row.source}</td>{/if}
  <td class="table__td table__td--node" title={row.path || row.nodeName}>{#if color}<span class="table__dot" style:background={color} aria-hidden="true"></span>{/if}{row.path || row.nodeName}</td>
  <td class="table__td table__td--pri"><Priority pri={row.item.pri} /></td>
  <td class="table__td table__td--status"><Pill phase={row.item.phase} /></td>
  <td class="table__td table__td--title"><span class="row__title">{row.item.title}</span></td>
  <td class="table__td table__td--due"><DueBadge item={row.item} {today} full /></td>
</tr>

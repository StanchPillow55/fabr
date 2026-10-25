<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  let { label, sub, value, onclick, lead }: { label: string; sub?: string; value?: string; onclick: () => void; lead?: Snippet } = $props();
</script>

<button type="button" class="row-item row-item--button" {onclick}>
  {#if lead}{@render lead()}{/if}
  <span class="row-label">{label}{#if sub}<span class="row-sub">{sub}</span>{/if}</span>
  {#if value}<span class="row-value">{value}</span>{/if}
  <Icon name="chevron-right" size={14} class="row-chev" />
</button>

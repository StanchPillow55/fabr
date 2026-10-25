<script lang="ts">
  let { pri }: { pri: number | null } = $props();
  const label = $derived(pri === 1 ? 'High priority' : pri === 2 ? 'Medium priority' : 'Low priority');
</script>

{#if pri && pri >= 1 && pri <= 3}<span class="pri pri--{pri}" title={label} aria-label={label}>{'!'.repeat(pri)}</span>{/if}

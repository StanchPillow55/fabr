<script lang="ts">
  import { unreadEvents, isUrgent, type FabricEvent } from './events';
  let { events, readPosition, onclick }: { events: FabricEvent[]; readPosition: number; onclick?: () => void } = $props();

  const unread = $derived(unreadEvents(events, readPosition));
  const newest = $derived(unread[0]);
</script>

{#if unread.length > 0}
  <button
    type="button"
    class="nbadge nbadge--{newest.kind}"
    class:nbadge--urgent={isUrgent(newest.kind)}
    aria-label="{unread.length} unread notices, newest: {newest.kind}"
    title="newest: {newest.kind}"
    onclick={() => onclick?.()}
  >
    <span class="nbadge__dot"></span>
    <span class="nbadge__count">{unread.length}</span>
    <span class="nbadge__kind">{newest.kind}</span>
  </button>
{/if}

<style>
  .nbadge { display: inline-flex; align-items: center; gap: 6px; padding: 2px 8px; border: 0; border-radius: 999px; background: var(--fill); color: var(--label-2); font-size: var(--fs-12); cursor: pointer; }
  .nbadge__dot { width: 8px; height: 8px; border-radius: 50%; background: var(--green); }
  .nbadge--failed .nbadge__dot { background: var(--red); }
  .nbadge--waiting .nbadge__dot, .nbadge--stuck .nbadge__dot { background: var(--orange); }
  .nbadge--urgent { color: var(--orange); font-weight: 600; }
  .nbadge__count { font-weight: 600; }
</style>

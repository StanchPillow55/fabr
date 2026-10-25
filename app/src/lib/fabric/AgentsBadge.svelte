<script lang="ts">
  import type { AgentView } from './stage';
  let { agents, onclick }: { agents: AgentView[]; onclick?: () => void } = $props();

  const needs = $derived(agents.filter((v) => v.waiting).length);
  const dead = $derived(agents.filter((v) => v.alive === 'dead').length);
  const tone = $derived(dead > 0 ? 'dead' : needs > 0 ? 'needs' : 'ok');
  const label = $derived(`${agents.length} ${agents.length === 1 ? 'agent' : 'agents'}${needs ? `, ${needs} need you` : ''}${dead ? `, ${dead} dead` : ''}`);
</script>

{#if agents.length > 0}
  <button type="button" class="abadge abadge--{tone}" aria-label={label} title={label} onclick={() => onclick?.()}>
    <span class="abadge__dot"></span>
    <span class="abadge__count">{agents.length}</span>
    <span class="abadge__kind">{agents.length === 1 ? 'agent' : 'agents'}</span>
  </button>
{/if}

<style>
  .abadge { display: inline-flex; align-items: center; gap: 6px; padding: 2px 8px; border: 0; border-radius: 999px; background: var(--fill); color: var(--label-2); font-size: var(--fs-12); cursor: pointer; }
  .abadge__dot { width: 8px; height: 8px; border-radius: 50%; background: var(--green); }
  .abadge--needs .abadge__dot { background: var(--orange); }
  .abadge--dead .abadge__dot { background: var(--red); }
  .abadge--needs, .abadge--dead { font-weight: 600; }
  .abadge--needs { color: var(--orange); }
  .abadge--dead { color: var(--red); }
  .abadge__count { font-weight: 600; }
</style>

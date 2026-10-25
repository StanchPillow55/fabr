<script lang="ts">
  import { errorParts } from '../model';

  let { message, onopen, compact = false }: { message: string; onopen: (url: string) => void; compact?: boolean } = $props();
  const parts = $derived(errorParts(message));
  const host = (u: string) => {
    try {
      return new URL(u).host;
    } catch {
      return u;
    }
  };
  const open = (e: Event, url: string) => {
    e.preventDefault();
    e.stopPropagation();
    onopen(url);
  };
</script>

{#if compact && parts.some((p) => p.url)}
  {#each parts.filter((p) => p.url) as p, i (i)}<a class="errlink" href={p.url} title={message} onclick={(e) => open(e, p.url!)}>Log in: {host(p.url!)}</a>{/each}
{:else}
  {#each parts as p, i (i)}{#if p.url}<a class="errlink" href={p.url} onclick={(e) => open(e, p.url!)}>{p.text}</a>{:else}{p.text}{/if}{/each}
{/if}

<style>
  .errlink { color: inherit; text-decoration: underline; cursor: pointer; }
</style>

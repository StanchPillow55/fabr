<script lang="ts">
  import Button from '../ui/Button.svelte';
  import { ageSeconds, elapsedSeconds, formatDuration, isDead, sortEntries, type RegistryEntry } from './agents';

  let { entries, now, titleFor, oncancel, onopen }: {
    entries: RegistryEntry[];
    now: Date | string;
    titleFor?: (taskPath: string) => string | undefined;
    oncancel?: (taskPath: string) => void;
    onopen?: (taskPath: string) => void;
  } = $props();

  let requested = $state<string[]>([]);
  let expanded = $state<string[]>([]);

  const sorted = $derived(sortEntries(entries));
  const present = $derived(new Set(entries.map((e) => e.task_path)));
  const isRequested = (p: string) => requested.includes(p) && present.has(p);

  $effect(() => {
    const gone = requested.filter((p) => !present.has(p));
    if (gone.length) requested = requested.filter((p) => present.has(p));
  });

  function cancel(p: string) {
    requested = [...requested, p];
    oncancel?.(p);
  }
  function toggle(p: string) {
    expanded = expanded.includes(p) ? expanded.filter((x) => x !== p) : [...expanded, p];
  }
  const questionText = (e: RegistryEntry) => (e.question && typeof e.question.text === 'string' ? e.question.text : null);
</script>

<section class="agents" aria-label="Agents">
  {#if sorted.length === 0}
    <p class="agents__empty">No agents running</p>
  {:else}
    <ul class="agents__rows">
      {#each sorted as e (e.task_path)}
        {@const dead = isDead(e, now)}
        {@const open = expanded.includes(e.task_path)}
        {@const lines = open ? e.log_tail : e.log_tail.slice(-3)}
        <li class="arow" class:arow--dead={dead} data-testid="agent-row">
          <header class="arow__head">
            <button type="button" class="arow__title" onclick={() => onopen?.(e.task_path)}>{titleFor?.(e.task_path) || e.task_id}</button>
            {#if dead}<span class="arow__dead">Dead {formatDuration(ageSeconds(e, now))}</span>{/if}
          </header>
          <dl class="arow__meta">
            <div><dt>Folder</dt><dd>{e.folder}</dd></div>
            <div><dt>Step</dt><dd>{e.step}</dd></div>
            <div><dt>Elapsed</dt><dd>{formatDuration(elapsedSeconds(e, now))}</dd></div>
            <div><dt>Cost</dt><dd>${e.cost_usd.toFixed(2)}</dd></div>
            <div><dt>Heartbeat</dt><dd>{formatDuration(ageSeconds(e, now))} ago</dd></div>
          </dl>
          {#if questionText(e)}<p class="arow__question">{questionText(e)}</p>{/if}
          {#if lines.length}
            <pre class="arow__log">{lines.join('\n')}</pre>
            {#if e.log_tail.length > 3}
              <button type="button" class="arow__more" onclick={() => toggle(e.task_path)}>{open ? 'show less' : `show all ${e.log_tail.length} lines`}</button>
            {/if}
          {/if}
          <footer class="arow__foot">
            {#if dead}
              <span class="arow__note">The backend removes stale entries on start.</span>
            {:else if isRequested(e.task_path)}
              <span class="arow__note">cancel requested</span>
            {:else}
              <Button variant="danger" onclick={() => cancel(e.task_path)}>Cancel</Button>
            {/if}
          </footer>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .agents__empty { color: var(--label-3); font-size: var(--fs-12); }
  .agents__rows { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  .arow { padding: 10px; border-radius: 8px; background: var(--fill); font-size: var(--fs-12); color: var(--label-2); }
  .arow--dead { border-left: 3px solid var(--red); }
  .arow__head { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .arow__title { border: 0; background: none; padding: 0; color: inherit; font-weight: 600; cursor: pointer; text-align: left; }
  .arow__dead { color: var(--red); font-weight: 600; }
  .arow__meta { display: flex; flex-wrap: wrap; gap: 4px 16px; margin: 6px 0; }
  .arow__meta div { display: flex; gap: 4px; }
  .arow__meta dt { color: var(--label-3); }
  .arow__meta dd { margin: 0; }
  .arow__question { margin: 6px 0; color: var(--orange); }
  .arow__log { margin: 6px 0; font-size: var(--fs-12); white-space: pre-wrap; overflow-wrap: anywhere; color: var(--label-2); }
  .arow__more { border: 0; background: none; padding: 0; color: var(--tint); cursor: pointer; font-size: var(--fs-12); }
  .arow__foot { margin-top: 6px; }
  .arow__note { color: var(--label-3); }
</style>

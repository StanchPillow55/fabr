<script lang="ts">
  import Button from '../ui/Button.svelte';
  import { formatDuration } from './agents';
  import StageBar from './StageBar.svelte';
  import { GROUPS, GROUP_LABEL, STAGES, groupOf, sinceSeconds, sortAgents, statusOf, type AgentView } from './stage';
  import { folderName } from './folder';

  let { agents, now, detail = false, oncancel, onopen }: {
    agents: AgentView[];
    now: Date | string;
    detail?: boolean;
    oncancel?: (taskPath: string) => void;
    onopen?: (recordPath: string) => void;
  } = $props();

  let requested = $state<string[]>([]);
  let expanded = $state<string[]>([]);

  const sorted = $derived(sortAgents(agents));
  const present = $derived(new Set(agents.map((v) => v.path)));
  const isRequested = (p: string) => requested.includes(p) && present.has(p);
  const canCancel = (v: AgentView) => v.stage === 'execute' || v.stage === 'evaluate';

  $effect(() => {
    const gone = requested.filter((p) => !present.has(p));
    if (gone.length) requested = requested.filter((p) => present.has(p));
  });

  function cancel(v: AgentView) {
    requested = [...requested, v.path];
    oncancel?.(v.path);
  }
  function toggle(p: string) {
    expanded = expanded.includes(p) ? expanded.filter((x) => x !== p) : [...expanded, p];
  }
  const questionText = (v: AgentView) => (v.entry?.question && typeof v.entry.question.text === 'string' ? v.entry.question.text : null);
  const usageRows = (v: AgentView) => STAGES.filter((s) => v.stageUsage?.[s]).map((s) => ({ stage: s, ...v.stageUsage![s] }));
</script>

<section class="agents" aria-label="Agents">
  {#if sorted.length === 0}
    <p class="agents__empty">No agents running</p>
  {:else if detail}
    <ul class="agents__rows">
      {#each sorted as v (v.path)}
        {@const log = v.entry?.log_tail ?? []}
        {@const open = expanded.includes(v.path)}
        {@const lines = open ? log : log.slice(-3)}
        {@const rows = usageRows(v)}
        <li class="agent" class:agent--dead={v.alive === 'dead'} data-testid="agent-row">
          <StageBar view={v} mode="full" {now} />
          {#if rows.length}
            <table class="agent__usage">
              <thead><tr><th>Stage</th><th>Calls</th><th>In</th><th>Out</th><th>Cost</th></tr></thead>
              <tbody>
                {#each rows as u (u.stage)}
                  <tr><td>{u.stage}</td><td>{u.calls}</td><td>{u.input_tokens}</td><td>{u.output_tokens}</td><td>${u.usd.toFixed(4)}</td></tr>
                {/each}
              </tbody>
            </table>
          {/if}
          {#if v.lastError}<p class="agent__error">{v.lastError}</p>{/if}
          {#if questionText(v)}<p class="agent__question">{questionText(v)}</p>{/if}
          {#if lines.length}
            <pre class="agent__log">{lines.join('\n')}</pre>
            {#if log.length > 3}
              <button type="button" class="agent__more" onclick={() => toggle(v.path)}>{open ? 'show less' : `show all ${log.length} lines`}</button>
            {/if}
          {/if}
          <footer class="agent__foot">
            {#if v.alive === 'dead'}
              <span class="agent__note">The backend removes stale entries on start.</span>
            {:else if isRequested(v.path)}
              <span class="agent__note">cancel requested</span>
            {:else if canCancel(v)}
              <Button variant="danger" onclick={() => cancel(v)}>Cancel</Button>
            {/if}
          </footer>
        </li>
      {/each}
    </ul>
  {:else}
    {#each GROUPS as g (g)}
      {@const list = sorted.filter((v) => groupOf(v) === g)}
      {#if list.length}
        <h3 class="agents__group agents__group--{g}">{GROUP_LABEL[g]} <span class="agents__n">{list.length}</span></h3>
        <ul class="agents__rows">
          {#each list as v (v.path)}
            <li>
              <button type="button" class="agent agent--row" class:agent--dead={v.alive === 'dead'} data-testid="agent-row" onclick={() => onopen?.(v.path)}>
                <span class="agent__title">{v.title}</span>
                <span class="agent__bar"><StageBar view={v} {now} /></span>
                <span class="agent__meta">{folderName(v.folder)} · {v.stage} · {statusOf(v, now)} · {formatDuration(sinceSeconds(v, now))}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/each}
  {/if}
</section>

<style>
  .agents__empty { color: var(--label-3); font-size: var(--fs-12); }
  .agents__group { margin: 12px 0 6px; font-size: var(--fs-12); font-weight: 600; color: var(--label-2); text-transform: uppercase; letter-spacing: 0.04em; }
  .agents__group--needs-you { color: var(--orange); }
  .agents__group--paused { color: var(--label-2); }
  .agents__group--dead { color: var(--red); }
  .agents__n { color: var(--label-3); font-weight: 400; }
  .agents__rows { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  .agent { display: block; width: 100%; padding: 10px; border: 0; border-radius: 8px; background: var(--fill); font-size: var(--fs-12); color: var(--label-2); text-align: left; }
  .agent--row { display: grid; gap: 6px; cursor: pointer; font: inherit; font-size: var(--fs-12); }
  .agent--dead { border-left: 3px solid var(--red); }
  .agent__title { color: var(--label); font-weight: 600; font-size: var(--fs-13); }
  .agent__bar { display: block; }
  .agent__meta { color: var(--label-3); }
  .agent__usage { width: 100%; margin-top: 8px; border-collapse: collapse; font-variant-numeric: tabular-nums; }
  .agent__usage th { font-weight: 400; color: var(--label-3); text-align: right; }
  .agent__usage td { text-align: right; }
  .agent__usage th:first-child, .agent__usage td:first-child { text-align: left; }
  .agent__error { margin: 6px 0; color: var(--error-text); overflow-wrap: anywhere; }
  .agent__question { margin: 6px 0; color: var(--orange); }
  .agent__log { margin: 6px 0; font-size: var(--fs-12); white-space: pre-wrap; overflow-wrap: anywhere; color: var(--label-2); }
  .agent__more { border: 0; background: none; padding: 0; color: var(--tint); cursor: pointer; font-size: var(--fs-12); }
  .agent__foot { margin-top: 6px; }
  .agent__note { color: var(--label-3); }
</style>

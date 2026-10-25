<script lang="ts">
  import { STAGES, ariaText, sinceSeconds, statusOf, usageLine, GATES, type AgentView } from './stage';
  import { formatDuration } from './agents';

  let { view, mode = 'compact', now = new Date() }: { view: AgentView; mode?: 'compact' | 'full'; now?: Date | string } = $props();

  const status = $derived(statusOf(view, now));
  const down = $derived(view.alive === 'dead' || view.alive === 'backend-down');
  const pulse = $derived(!view.finished && !view.suspended && view.alive === 'live' && !view.waiting && (view.activity === 'model' || view.activity === 'tool' || view.activity === 'check'));
  const tone = $derived(view.suspended ? 'off' : view.activity === 'paused' && view.alive === 'live' ? 'paused' : view.finished === 'failed' ? 'bad' : view.alive === 'dead' ? 'bad' : view.alive === 'backend-down' ? 'off' : view.waiting ? 'wait' : view.activity === 'retry' ? 'retry' : 'ok');

  function seg(i: number): string {
    if (view.finished === 'succeeded') return 'done';
    if (i < view.index) return 'done';
    return i === view.index ? 'cur' : 'future';
  }
</script>

<div class="sbar sbar--{mode} sbar--{tone}" class:sbar--down={down} role="progressbar" aria-valuemin={0} aria-valuemax={STAGES.length} aria-valuenow={view.index + 1} aria-valuetext={ariaText(view, now)} title={view.lastError ?? undefined}>
  <ol class="sbar__row">
    {#each STAGES as s, i (s)}
      <li class="sbar__seg sbar__seg--{seg(i)}" class:sbar__seg--gate={GATES.includes(s)} class:sbar__seg--pulse={i === view.index && pulse} title={s}>
        {#if mode === 'full'}<span class="sbar__lab">{i === view.index && view.waiting ? 'needs you' : s}</span>{/if}
      </li>
    {/each}
  </ol>
  {#if mode === 'full'}
    <p class="sbar__line">
      <b class="sbar__status">{status}</b>
      {#if !view.finished}<span>in {view.stage} for {formatDuration(sinceSeconds(view, now))}</span>{/if}
      <span class="sbar__usage">{usageLine(view)}</span>
    </p>
  {/if}
</div>

<style>
  .sbar { display: block; width: 100%; --seg: var(--tint); }
  .sbar--wait { --seg: var(--orange); }
  .sbar--bad { --seg: var(--red); }
  .sbar--off { --seg: var(--label-3); }
  .sbar--paused { --seg: var(--label-2); }
  .sbar--down { opacity: 0.7; }
  .sbar__row { display: flex; align-items: center; gap: 2px; list-style: none; margin: 0; padding: 0; }
  .sbar__seg { flex: 1; min-width: 0; height: 4px; border-radius: var(--radius-pill); background: var(--fill-2); text-align: center; }
  .sbar--full .sbar__seg { height: 18px; display: flex; align-items: center; justify-content: center; }
  .sbar__seg--gate { height: 6px; box-shadow: 0 0 0 1px var(--label-3); }
  .sbar--full .sbar__seg--gate { height: 22px; }
  .sbar__seg--done { background: color-mix(in srgb, var(--tint) 55%, transparent); }
  .sbar--off .sbar__seg--done { background: var(--label-3); }
  .sbar__seg--cur { background: var(--seg); }
  .sbar--retry .sbar__seg--cur { background: repeating-linear-gradient(45deg, var(--orange) 0 4px, var(--fill) 4px 8px); }
  .sbar--wait .sbar__seg--cur.sbar__seg--gate { box-shadow: 0 0 0 1px var(--orange); }
  .sbar__seg--pulse { animation: sbar-pulse 1.4s ease-in-out infinite; }
  @keyframes sbar-pulse { 50% { opacity: 0.45; } }
  @media (prefers-reduced-motion: reduce) { .sbar__seg--pulse { animation: none; } }
  .sbar__lab { font-size: var(--fs-11); line-height: 1; color: var(--label-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sbar__seg--cur .sbar__lab { color: var(--on-tint); font-weight: 600; }
  .sbar--retry .sbar__seg--cur .sbar__lab { color: var(--label); }
  .sbar__line { display: flex; flex-wrap: wrap; gap: 4px 12px; margin: 6px 0 0; font-size: var(--fs-12); color: var(--label-2); }
  .sbar--paused .sbar__status { color: var(--label-2); }
  .sbar--wait .sbar__status { color: var(--orange); }
  .sbar--bad .sbar__status { color: var(--red); }
  .sbar--retry .sbar__status { color: var(--orange); }
  .sbar__usage { margin-left: auto; color: var(--label-3); font-variant-numeric: tabular-nums; }
</style>

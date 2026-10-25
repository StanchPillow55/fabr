<script lang="ts">
  import { PHASE_LABEL, type Phase } from './phase';
  import type { State, Step } from './types';
  let { state, step, phase }: { state: State; step: Step | null; phase: Phase } = $props();

  const stages = $derived([
    { id: 'reminder', label: 'reminder' },
    { id: 'task', label: 'task' },
    { id: 'defined', label: 'defined' },
    { id: 'in_progress', label: 'in progress' },
    state === 'failed' ? { id: 'failed', label: 'failed' } : { id: 'succeeded', label: 'succeeded' },
  ]);
  const at = $derived(stages.findIndex((s) => s.id === state));
  const showStep = $derived(step && (state === 'task' || state === 'in_progress' || state === 'defined'));
</script>

<div class="ribbon" aria-label="State: {state}, {PHASE_LABEL[phase]}">
  <ol class="ribbon__row">
    {#each stages as s, i (s.id)}
      <li class="ribbon__stage" class:ribbon__stage--past={i < at} class:ribbon__stage--now={i === at} class:ribbon__stage--failed={s.id === 'failed'} aria-current={i === at ? 'step' : undefined}>{s.label}</li>
    {/each}
  </ol>
  <p class="ribbon__sub">
    <span class="ribbon__phase ribbon__phase--{phase}" data-testid="phase">{PHASE_LABEL[phase]}</span>
    {#if showStep}<span>step: <b>{step}</b></span>{/if}
  </p>
</div>

<style>
  .ribbon__row { display: flex; gap: 4px; list-style: none; margin: 0; padding: 0; }
  .ribbon__stage { flex: 1; text-align: center; padding: 6px 4px; font-size: var(--fs-12); border-radius: var(--radius-ctl); background: var(--fill); color: var(--label-3); }
  .ribbon__stage--past { color: var(--label-2); }
  .ribbon__stage--now { background: var(--tint); color: var(--on-tint); font-weight: 600; }
  .ribbon__stage--now.ribbon__stage--failed { background: var(--red); }
  .ribbon__sub { display: flex; gap: 12px; margin: 8px 0 0; min-height: 1.2em; font-size: var(--fs-12); color: var(--label-2); }
  .ribbon__phase { font-weight: 600; color: var(--tint); }
  .ribbon__phase--needs_you { color: var(--orange); }
  .ribbon__phase--paused { color: var(--label-2); }
  .ribbon__phase--suspended { color: var(--label-3); font-style: italic; }
  .ribbon__phase--failed { color: var(--red); }
  .ribbon__phase--done { color: var(--status-done); }
  .ribbon__phase--todo { color: var(--status-todo); }
</style>

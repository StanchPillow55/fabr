<script lang="ts">
  import Button from '../ui/Button.svelte';
  import Group from '../ui/Group.svelte';
  import Page from '../ui/Page.svelte';
  import type { AnswerInput } from './answer';
  import ApprovalPanel from './ApprovalPanel.svelte';
  import type { ApprovalDecision } from './approval';
  import { describeAnswer } from './answer';
  import { splitContext } from './parse';
  import QuestionPanel from './QuestionPanel.svelte';
  import { backendSilent, pendingRequests, requestFor, type RequestKey } from './requests';
  import StateRibbon from './StateRibbon.svelte';
  import type { TaskRecord } from './types';

  let { record, name, onback, onanswer, onapprove, onrequest, now = new Date(), onreload }: {
    record: TaskRecord;
    name: string;
    onback?: () => void;
    onanswer?: (a: AnswerInput) => Promise<void>;
    onapprove?: (d: ApprovalDecision) => Promise<void>;
    onrequest?: (key: RequestKey) => Promise<void>;
    now?: Date | string;
    onreload?: () => Promise<void> | void;
  } = $props();

  const labels: Record<RequestKey, string> = { delegate: 'Delegate', cancel: 'Cancel', redelegate: 'Re-delegate' };
  const wanted = $derived(requestFor(record.state));
  const canAsk = $derived(wanted !== null && !record.requests[wanted] && !(wanted === 'redelegate' && record.links.redelegated_to));
  const queued = $derived(pendingRequests(record));
  const silent = $derived(backendSilent(record, now));
  let busy = $state(false);
  let reqError = $state<string | null>(null);
  async function ask(key: RequestKey) {
    busy = true;
    reqError = null;
    try { await onrequest?.(key); }
    catch (e) { reqError = (e as Error).message; }
    finally { busy = false; }
  }

  const s = $derived(record.sections);
  const ctx = $derived(splitContext(s.context));
  const items = $derived(new Map((record.verdict?.items ?? []).map((i) => [i.id, i])));
  const mark = (id: string) => {
    const i = items.get(id);
    return i ? (i.pass ? 'pass' : 'fail') : 'unchecked';
  };
  const symbol = { pass: '✓', fail: '✗', unchecked: '–' } as const;
  const q = $derived(record.question);
  const ap = $derived(record.approval);
  const pm = $derived(record.plan_meta);
</script>

<Page title={record.title} back="Records" {onback}>
  <div class="rec">
    <header class="rec__head">
      <h1 class="rec__title">{record.title}</h1>
      <p class="rec__meta">{name}{#if record.due} · due {record.due}{/if} · cost ${record.cost_usd}</p>
    </header>

    <StateRibbon state={record.state} step={record.step} waiting={record.waiting} />

    {#if canAsk || queued.length}
      <div class="req">
        {#if canAsk && wanted}
          <Button variant={wanted === 'cancel' ? 'danger' : 'primary'} disabled={busy} onclick={() => ask(wanted)}>{labels[wanted]}</Button>
        {/if}
        {#each queued as k (k)}<span class="req__queued">{labels[k]}: queued</span>{/each}
        {#if silent}<span class="req__silent">agent backend not connected</span>{/if}
        {#if reqError}<span class="req__err">{reqError}</span>{/if}
      </div>
    {/if}

    <Group header="Context">
      <div class="blk">
        {#if ctx.own}<p class="txt">{ctx.own}</p>{:else}<p class="slot">Context: written by you when the task is created</p>{/if}
        {#if ctx.gathered}<h4 class="sub">Gathered</h4><p class="txt">{ctx.gathered}</p>
        {:else}<p class="slot">Gathered: appended by the gatherer during gather</p>{/if}
      </div>
    </Group>

    <Group header="Spec">
      <div class="blk">{#if s.spec}<p class="txt">{s.spec}</p>{:else}<p class="slot">Spec: written by the definer during define</p>{/if}</div>
    </Group>

    <Group header="Success">
      <div class="blk">
        {#if s.success}<p class="txt">{s.success}</p>{:else}<p class="slot">Success: the success definition, written by you</p>{/if}
        {#if record.assertions.length}
          <ul class="list">
            {#each record.assertions as a (a.id)}
              <li class="assert"><span class="mark mark--{mark(a.id)}" title={mark(a.id)}>{symbol[mark(a.id)]}</span><span><b>{a.id}</b> {a.text} <span class="dim">({mark(a.id)})</span></span></li>
            {/each}
          </ul>
        {:else}<p class="slot">Assertions: the checkable form, written by the definer during define</p>{/if}
      </div>
    </Group>

    {#if q}
      <QuestionPanel question={q} onsubmit={onanswer} {onreload} />
    {/if}

    {#if record.answers.length}
      <Group header="Answers">
        <div class="blk">
          <ul class="list">
            {#each record.answers as a (a.id)}
              <li class="txt"><b>{a.id}</b> ({a.kind}) {a.text}<br /><span class="dim">{describeAnswer(a)}{#if a.answer} · by {a.answer.by} at {a.answer.at}{/if}</span></li>
            {/each}
          </ul>
        </div>
      </Group>
    {/if}

    <Group header="Plan">
      <div class="blk">
        {#if s.plan}<p class="txt">{s.plan}</p>{:else}<p class="slot">Plan: written by the planner during plan</p>{/if}
        {#if pm}
          <p class="dim">reversibility: <b class:warn={pm.reversibility === 'irreversible'}>{pm.reversibility}</b> · tools: {pm.tools.join(', ') || 'none'}</p>
          {#if pm.assumptions.length}
            <ul class="list">{#each pm.assumptions as a (a.rule_id)}<li>assumes <b>{a.rule_id}</b>: {a.text}</li>{/each}</ul>
          {/if}
        {/if}
      </div>
    </Group>

    {#if ap}
      <ApprovalPanel approval={ap} grants={record.grants} planMeta={pm} plan={s.plan} assertions={record.assertions} onsubmit={onapprove} {onreload} />
    {:else}
      <Group header="Grants">
        <div class="blk">
          <p class="txt">requested: {record.grants.requested.join(', ') || 'none'}</p>
          <p class="txt">granted: {record.grants.granted.map((g) => `${g.field} (${g.scope})`).join(', ') || 'none'}</p>
        </div>
      </Group>
      <Group header="Approval">
        <div class="blk"><p class="slot">Approval: requested by the backend after plan</p></div>
      </Group>
    {/if}

    <Group header="Log">
      <div class="blk">
        {#if s.logEntries.length}
          <ul class="list">
            {#each s.logEntries as e, i (i)}<li class="log"><span class="dim">{e.ts}</span> <b>{e.step}</b> {e.message}</li>{/each}
          </ul>
        {:else}<p class="slot">Log: written by the backend as each step runs</p>{/if}
      </div>
    </Group>

    <Group header="Verdict">
      <div class="blk">
        {#if record.verdict}
          <p class="txt"><b class:warn={record.verdict.result === 'fail'}>{record.verdict.result}</b> at {record.verdict.at}</p>
          <ul class="list">
            {#each record.verdict.items as i (i.id)}
              <li class="assert"><span class="mark mark--{i.pass ? 'pass' : 'fail'}">{i.pass ? '✓' : '✗'}</span><span><b>{i.id}</b> {i.evidence}</span></li>
            {/each}
          </ul>
        {:else}<p class="slot">Verdict: written by the backend during evaluate</p>{/if}
      </div>
    </Group>

    {#if record.links.redelegated_from || record.links.redelegated_to}
      <Group header="Links">
        <div class="blk">
          {#if record.links.redelegated_from}<p class="txt">re-delegated from {record.links.redelegated_from}</p>{/if}
          {#if record.links.redelegated_to}<p class="txt">re-delegated to {record.links.redelegated_to}</p>{/if}
        </div>
      </Group>
    {/if}
  </div>
</Page>

<style>
  .rec { display: flex; flex-direction: column; gap: 18px; padding: 8px 0 24px; }
  .rec__title { margin: 0; font-size: 20px; }
  .rec__meta { margin: 4px 0 0; font-size: var(--fs-12); color: var(--label-2); }
  .blk { padding: 10px 16px; display: flex; flex-direction: column; gap: 6px; }
  .txt { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
  .dim { color: var(--label-2); font-size: var(--fs-12); margin: 0; }
  .slot { margin: 0; color: var(--label-3); font-style: italic; border: 1px dashed var(--sep); border-radius: var(--radius-ctl); padding: 6px 10px; }
  .sub { margin: 4px 0 0; font-size: var(--fs-12); color: var(--label-2); text-transform: uppercase; }
  .list { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 4px; }
  .assert { display: flex; gap: 8px; }
  .mark { width: 1.2em; text-align: center; font-weight: 700; color: var(--label-3); }
  .mark--pass { color: var(--green); }
  .mark--fail { color: var(--red); }
  .req { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .req__queued { color: var(--label-2); font-size: var(--fs-12); }
  .req__silent { color: var(--orange); font-size: var(--fs-12); }
  .req__err { color: var(--red); font-size: var(--fs-12); }
  .warn { color: var(--orange); }
  .log { font-size: var(--fs-12); overflow-wrap: anywhere; }
</style>

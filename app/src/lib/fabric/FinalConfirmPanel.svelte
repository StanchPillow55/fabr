<script lang="ts">
  import Button from '../ui/Button.svelte';
  import Group from '../ui/Group.svelte';
  import { finalConfirmActions, finalConfirmVerb, type FinalConfirmDecision } from './approval';
  import type { Approval, BoundAction, TaskRecord } from './types';
  import { ConflictError } from './write';

  let { record, onsubmit, onreload }: {
    record: TaskRecord;
    onsubmit?: (d: FinalConfirmDecision) => Promise<void>;
    onreload?: () => Promise<void> | void;
  } = $props();

  const confirm = $derived<Approval | null>(record.final_confirm);
  const actions = $derived<BoundAction[]>(finalConfirmActions(record));
  const verb = $derived(finalConfirmVerb(record));
  const verbLabel = $derived(verb === 'pay' ? 'pay' : 'send');

  let note = $state('');
  let cancelling = $state(false);
  let busy = $state(false);
  let error = $state('');
  let conflict = $state(false);

  // Parse the canonical-JSON params into a readable, ordered field list. The
  // backend stores `params` as a canonical JSON string; render it as-is when it
  // is not an object (so nothing is ever hidden from the person confirming).
  interface Field { key: string; value: string; long: boolean }
  const PREFERRED = ['to', 'cc', 'bcc', 'recipient', 'from', 'subject', 'amount', 'currency', 'commit_message', 'message', 'branch', 'repo', 'body', 'diff', 'content', 'text'];
  const LONG = new Set(['body', 'diff', 'content', 'text', 'message', 'commit_message']);

  function fieldsOf(a: BoundAction): { fields: Field[]; raw: string | null } {
    let parsed: unknown;
    try {
      parsed = JSON.parse(a.params);
    } catch {
      return { fields: [], raw: a.params };
    }
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return { fields: [], raw: a.params };
    }
    const obj = parsed as Record<string, unknown>;
    const keys = Object.keys(obj);
    keys.sort((x, y) => {
      const ix = PREFERRED.indexOf(x);
      const iy = PREFERRED.indexOf(y);
      if (ix === iy) return x.localeCompare(y);
      if (ix < 0) return 1;
      if (iy < 0) return -1;
      return ix - iy;
    });
    const fields = keys.map((k) => {
      const v = obj[k];
      const value = typeof v === 'string' ? v : JSON.stringify(v, null, 2);
      return { key: k, value, long: LONG.has(k) || value.includes('\n') || value.length > 80 };
    });
    return { fields, raw: null };
  }

  async function send(d: FinalConfirmDecision) {
    if (!onsubmit || busy) return;
    busy = true;
    error = '';
    conflict = false;
    try {
      await onsubmit(d);
    } catch (e) {
      conflict = e instanceof ConflictError;
      error = conflict ? 'The record changed on disk since you opened it, so nothing was written.' : (e as Error).message;
    } finally {
      busy = false;
    }
  }

  const doConfirm = () => send({ status: 'approved' });
  const doCancel = () => send({ status: 'refused', note });

  async function reload() {
    error = '';
    conflict = false;
    await onreload?.();
  }
</script>

{#if confirm}
  <Group header="Final confirmation">
    <div class="fc" data-testid="final-confirm">
      {#if confirm.status === 'pending'}
        <section class="gate">
          <h3 class="title" role="alert" data-testid="final-confirm-title">Final confirmation — this will {verbLabel}</h3>
          <p class="dim">This is the last layer. Once you confirm, the external effect runs and cannot be undone. Review the exact artifact below.</p>
        </section>

        {#if actions.length}
          {#each actions as a, i (i)}
            {@const parsed = fieldsOf(a)}
            <section class="artifact" data-testid="final-confirm-artifact">
              <h4>{a.effect === 'payment' ? 'Payment' : 'Action'}: {a.tool}</h4>
              {#if a.destination}<p class="dest"><span class="k">to</span> <b>{a.destination}</b></p>{/if}
              {#if parsed.fields.length}
                <dl class="kv">
                  {#each parsed.fields as f (f.key)}
                    <dt>{f.key}</dt>
                    {#if f.long}
                      <dd><pre class="body">{f.value}</pre></dd>
                    {:else}
                      <dd>{f.value}</dd>
                    {/if}
                  {/each}
                </dl>
              {:else if parsed.raw}
                <pre class="body" data-testid="final-confirm-raw">{parsed.raw}</pre>
              {:else}
                <p class="dim">No parameters.</p>
              {/if}
            </section>
          {/each}
        {:else}
          <section class="artifact">
            <p class="dim">The bound action details are not available on this record.</p>
          </section>
        {/if}

        {#if !onsubmit}
          <p class="dim">Waiting for a decision.</p>
        {:else}
          <div class="row">
            <Button variant="primary" disabled={busy} onclick={doConfirm} data-testid="final-confirm-send">Confirm &amp; {verbLabel}</Button>
            <Button variant="danger" on={cancelling} disabled={busy} onclick={() => (cancelling = !cancelling)} data-testid="final-confirm-cancel">Cancel</Button>
          </div>
          {#if cancelling}
            <textarea class="in" rows="2" aria-label="Cancellation note" placeholder="Why (optional)" bind:value={note} disabled={busy}></textarea>
            <div class="row"><Button variant="danger" disabled={busy} onclick={doCancel}>Confirm cancellation</Button></div>
          {/if}
          {#if busy}<p class="dim" role="status">Saving…</p>{/if}
          {#if error}
            <p class="err" role="alert">{error}</p>
            {#if conflict}<div class="row"><Button onclick={reload}>Reload</Button></div>{/if}
          {/if}
        {/if}
      {:else}
        <p class="txt" data-testid="final-confirm-decided"><b class:warn={confirm.status === 'refused'}>{confirm.status === 'refused' ? 'cancelled' : 'confirmed'}</b>{#if confirm.by} by {confirm.by}{/if}{#if confirm.at} at {confirm.at}{/if}</p>
        {#if confirm.note}<p class="dim">Note: {confirm.note}</p>{/if}
      {/if}
    </div>
  </Group>
{/if}

<style>
  .fc { padding: 10px 16px; display: flex; flex-direction: column; gap: 12px; }
  section { display: flex; flex-direction: column; gap: 4px; }
  .gate { gap: 6px; border-left: 3px solid var(--orange); padding-left: 10px; }
  .title { margin: 0; font-size: var(--fs-14); color: var(--orange); font-weight: 700; }
  .artifact { border: 1px solid var(--sep); border-radius: var(--radius-ctl); padding: 10px 12px; gap: 6px; }
  h4 { margin: 0; font-size: var(--fs-12); color: var(--label-2); text-transform: uppercase; }
  .dest { margin: 0; overflow-wrap: anywhere; }
  .k { color: var(--label-2); font-size: var(--fs-12); }
  .txt { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
  .dim { color: var(--label-2); font-size: var(--fs-12); margin: 0; overflow-wrap: anywhere; }
  .warn { color: var(--orange); font-weight: 600; }
  .err { margin: 0; color: var(--red); }
  .kv { margin: 0; display: grid; grid-template-columns: max-content 1fr; gap: 2px 10px; }
  .kv dt { color: var(--label-2); font-size: var(--fs-12); }
  .kv dd { margin: 0; overflow-wrap: anywhere; min-width: 0; }
  .kv dd:has(.body) { grid-column: 1 / -1; }
  .body { margin: 2px 0 0; white-space: pre-wrap; overflow-wrap: anywhere; font: inherit; background: var(--bg-2, transparent); padding: 6px 8px; border-radius: var(--radius-ctl); }
  .row { display: flex; gap: 8px; flex-wrap: wrap; }
  .in { width: 100%; box-sizing: border-box; font: inherit; padding: 8px 10px; border: 1px solid var(--sep); border-radius: var(--radius-ctl); background: transparent; color: inherit; }
</style>

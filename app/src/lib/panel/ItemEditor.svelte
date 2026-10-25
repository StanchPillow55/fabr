<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import type { Item, Status } from '../api/types';
  import { dueDate, dueTime, joinDue } from '../model';
  import { STATUSES } from '../store.svelte';
  import Icon from '../ui/Icon.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import { appendItem, commitInput, patchItem, removeItem, titleDecision } from './itemcommit';

  let { items, oncommit, onfocus }: {
    items: Item[];
    oncommit: (mutate: (items: Item[]) => Item[] | null) => Promise<boolean>;
    onfocus?: (on: boolean) => void;
  } = $props();

  const PRI = [
    { value: '0', label: '–', title: 'No priority' },
    { value: '1', label: '!', title: 'High' },
    { value: '2', label: '!!', title: 'Medium' },
    { value: '3', label: '!!!', title: 'Low' },
  ];
  let list = $state<HTMLDivElement>();
  let draft = $state<Item | null>(null);
  const sending = new Set<string>();

  function focusIn(e: FocusEvent) {
    if ((e.target as HTMLElement).matches('input, select')) onfocus?.(true);
  }
  function focusOut(e: FocusEvent) {
    const to = e.relatedTarget as HTMLElement | null;
    if (to && list?.contains(to) && to.matches('input, select')) return;
    onfocus?.(false);
  }
  onDestroy(() => onfocus?.(false));

  function patch(i: number, p: Partial<Item>) {
    return oncommit(patchItem(i, p));
  }
  async function once(id: string, run: () => Promise<unknown>) {
    if (sending.has(id)) return;
    sending.add(id);
    try {
      await run();
    } finally {
      sending.delete(id);
    }
  }

  function title(i: number, el: HTMLInputElement) {
    const d = titleDecision(items[i]?.title ?? '', el.value);
    if (d.kind !== 'save') {
      el.value = d.title;
      return;
    }
    return once(`t${i}:${d.title}`, () => patch(i, { title: d.title }));
  }
  function status(i: number, v: Status) {
    if (items[i] && items[i].status !== v) patch(i, { status: v });
  }
  function pri(i: number, v: string) {
    const n = v === '0' ? null : Number(v);
    if (items[i] && (items[i].pri ?? null) !== n) patch(i, { pri: n });
  }
  function due(i: number, next: string | null) {
    if (items[i] && (items[i].due ?? null) !== next) patch(i, { due: next });
  }
  function dateChanged(i: number, el: HTMLInputElement) {
    const t = items[i]?.due ? dueTime(items[i].due!) : null;
    due(i, el.value ? joinDue(el.value, t) : null);
  }
  function timeChanged(i: number, el: HTMLInputElement) {
    const d = items[i]?.due;
    if (d) due(i, joinDue(dueDate(d), el.value || null));
  }
  function del(i: number) {
    oncommit(removeItem(i));
  }
  function enter(e: KeyboardEvent, run: () => unknown) {
    if (e.key !== 'Enter') return;
    e.preventDefault();
    run();
  }

  export async function add() {
    if (!draft) draft = { id: null, status: 'todo', due: null, title: '', pri: null };
    await tick();
    list?.querySelector<HTMLInputElement>('.erow--draft .erow__title')?.focus();
  }
  function draftTitle(el: HTMLInputElement, to: EventTarget | null = null) {
    const d = draft;
    if (!d) return;
    const v = el.value.trim();
    if (!v) {
      if (!(to instanceof HTMLElement && el.closest('.erow')?.contains(to))) draft = null;
      return;
    }
    return once('draft', async () => {
      const row: Item = { ...d, title: v };
      if (await oncommit(appendItem(row))) draft = null;
    });
  }
</script>

{#snippet fields(item: Item, setStatus: (v: Status) => void, setPri: (v: string) => void, setDate: (el: HTMLInputElement) => void, setTime: (el: HTMLInputElement) => void, clear: () => void, remove: () => void)}
  <div class="erow__meta">
    <select class="erow__status status--{item.status}" aria-label="Status" value={item.status} onchange={(e) => setStatus(e.currentTarget.value as Status)}>
      {#each STATUSES as s (s)}<option value={s}>{s}</option>{/each}
    </select>
    <Segmented size="sm" label="Priority" options={PRI} value={String(item.pri ?? 0)} onchange={setPri} />
    <span class="erow__date">
      <input type="date" aria-label="Due date" value={item.due ? dueDate(item.due) : ''} onchange={(e) => setDate(e.currentTarget)} />
      {#if item.due}
        <input class="erow__time" type="time" aria-label="Due time (optional)" value={dueTime(item.due) ?? ''} onchange={(e) => setTime(e.currentTarget)} />
        <button type="button" class="btn btn--icon erow__clear" title="Clear date" onclick={clear}><Icon name="x" size={12} /></button>
      {/if}
    </span>
    <button type="button" class="btn btn--icon btn--danger erow__del" title="Delete item" aria-label="Delete item" onclick={remove}><Icon name="trash" size={14} /></button>
  </div>
{/snippet}

<div class="group" bind:this={list} onfocusin={focusIn} onfocusout={focusOut}>
  {#each items as item, i (i)}
    <div class="erow row--{item.status}">
      <input
        class="erow__title"
        type="text"
        value={item.title}
        placeholder="Title"
        aria-label="Title"
        {@attach commitInput((el) => title(i, el))}
      />
      {@render fields(item, (v) => status(i, v), (v) => pri(i, v), (el) => dateChanged(i, el), (el) => timeChanged(i, el), () => due(i, null), () => del(i))}
    </div>
  {/each}
  {#if draft}
    {@const d = draft}
    <div class="erow erow--draft row--{d.status}">
      <input
        class="erow__title"
        type="text"
        value=""
        placeholder="Title"
        aria-label="Title"
        onkeydown={(e) => enter(e, () => draftTitle(e.currentTarget))}
        onblur={(e) => draftTitle(e.currentTarget, e.relatedTarget)}
      />
      {@render fields(d, (v) => (d.status = v), (v) => (d.pri = v === '0' ? null : Number(v)),
        (el) => { d.due = el.value ? joinDue(el.value, d.due ? dueTime(d.due) : null) : null; },
        (el) => { if (d.due) d.due = joinDue(dueDate(d.due), el.value || null); },
        () => (d.due = null), () => (draft = null))}
    </div>
  {:else if !items.length}
    <p class="group__empty">No items yet.</p>
  {/if}
</div>

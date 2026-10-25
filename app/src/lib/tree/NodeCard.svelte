<script lang="ts">
  import type { Item, Node, Source, SourceError } from '../api/types';
  import { MAX_ROWS } from '../constants';
  import { cardAside, cardTitle, dueLabel, isHidden, isVirtualRoot, rollup, rootCounts, sourceLines, type BoxModel, type Key } from '../model';
  import ErrorText from '../ui/ErrorText.svelte';
  import DueBadge from '../ui/DueBadge.svelte';
  import Icon from '../ui/Icon.svelte';
  import Pill from '../ui/Pill.svelte';
  import Priority from '../ui/Priority.svelte';
  import AddTask from '../fabric/AddTask.svelte';
  import { canMarkDone } from '../fabric/done';
  import TaskStrip from '../fabric/TaskStrip.svelte';
  import type { FabricStore, Held } from '../fabric/tasks.svelte';

  interface Props {
    node: Node;
    tally?: Node;
    model: BoxModel;
    selected: boolean;
    match: boolean;
    dim: boolean;
    ontoggle: () => void;
    onmore: () => void;
    onopen: () => void;
    onfocus: () => void;
    today?: string;
    accent?: string | null;
    source?: Source | null;
    sources?: Source[];
    errors?: SourceError[];
    pending?: string[];
    onsource?: (key: Key) => void;
    onurl?: (url: string) => void;
    fabric?: FabricStore | null;
    folder?: string | null;
    remote?: boolean;
  }
  let {
    node, tally, model, selected, match, dim, ontoggle, onmore, onopen, onfocus, today = '', source = null, accent = null,
    sources = [], errors = [], pending = [], onsource, onurl, fabric = null, folder = null, remote = false,
  }: Props = $props();

  const hidden = $derived(isHidden(node));
  const virtual = $derived(isVirtualRoot(node));
  const title = $derived(cardTitle(node));
  const aside = $derived(cardAside(node, source));
  const counts = $derived(virtual && !model.collapsed ? rootCounts(tally ?? node, today) : null);
  const lines = $derived(virtual && !model.collapsed ? sourceLines(tally ?? node, sources, errors, today, undefined, pending) : []);
  const canToggle = $derived(node.children.length > 0 || node.items.length > 0);
  const roll = $derived(model.collapsed ? rollup(tally ?? node, today) : null);
  const REMOTE_NOTE = 'Remote node: read-only';
  const expandedMore = $derived(model.shown.length > MAX_ROWS);

  const held = (item: Item) => (fabric && folder ? fabric.tasksFor(folder).find((h) => h.record.id === item.id) : undefined);
  function openItem(item: Item) {
    const h = held(item);
    onopen();
    if (h && fabric) fabric.open(h.path);
  }
  async function markDone(h: Held) {
    try {
      await fabric?.markDone(h.path);
    } catch (e) {
      if (fabric) fabric.actionError = (e as Error).message;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.target !== e.currentTarget) return;
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onopen();
    }
  }
  const stop = (fn: () => void) => (e: Event) => {
    e.stopPropagation();
    fn();
  };
</script>

<div
  class="card node"
  class:card--plain={!node.managed && !virtual}
  class:card--root={virtual}
  class:card--selected={selected}
  class:card--match={match}
  class:card--dim={dim}
  class:card--hidden={hidden}
  class:card--accent={!!accent}
  style:--accent={accent}
  role="button"
  tabindex="0"
  aria-label={title}
  onclick={onopen}
  {onkeydown}
>
  <div class="card__head" role="presentation" ondblclick={stop(onfocus)} title={node.path || title}>
    {#if canToggle}
      <button type="button" class="chev" title={model.collapsed ? 'Expand' : 'Collapse'} aria-expanded={!model.collapsed} onclick={stop(ontoggle)} ondblclick={(e) => e.stopPropagation()}>
        <Icon name={model.collapsed ? 'chevron-right' : 'chevron-down'} size={12} />
      </button>
    {:else}
      <span class="chev chev--none"></span>
    {/if}
    {#if source}<span class="card__src"><Icon name={source.host ? 'server' : 'folder'} size={13} /></span>{/if}
    <span class="card__name">{title}</span>
    {#if hidden}
      <span class="card__hidden" title="Hidden node"><Icon name="eye-off" size={12} /></span>
    {/if}
    {#if node.warnings.length}
      <span class="card__warn" title={node.warnings.join('\n')}><Icon name="warning" size={12} /></span>
    {/if}
    {#if fabric && folder}<TaskStrip {fabric} {folder} />{/if}
    {#if aside}<span class="card__kind">{aside}</span>{/if}
  </div>

  {#if counts}
    <ul class="card__items">
      <li class="row row--counts">
        <span><b>{counts.todo}</b> todo</span><span class="dotsep">·</span>
        <span class:card__wip={counts.active > 0}><b>{counts.active}</b> active</span><span class="dotsep">·</span>
        <span class:card__overdue={counts.overdue > 0}><b>{counts.overdue}</b> overdue</span>
      </li>
      {#each lines as l (l.key)}
        <li class="row row--source">
          <button type="button" class="srcline" title={l.error ?? `Open ${l.name}`} onclick={stop(() => onsource?.(l.key))}>
            <span class="dot" class:dot--ok={!l.error && !l.pending} class:dot--wait={!l.error && l.pending} class:dot--bad={!!l.error}></span>
            <span class="row__title">{l.name} · {l.where}</span>
            {#if l.error}<span class="srcline__error"><ErrorText message={l.error} compact onopen={(u) => onurl?.(u)} /></span>{:else if l.pending}<span class="srcline__error srcline__pending">Connecting…</span>{:else}<span class="srcline__open">{l.open} open</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
  {:else if node.managed && !model.collapsed}
    <ul class="card__items">
      {#if fabric}
        <li class="row row--add">
          <AddTask {fabric} {folder} variant="card" disabled={remote} note={remote ? REMOTE_NOTE : ''} />
        </li>
      {/if}
      {#each model.shown as item}
        {@const h = held(item)}
        <li class="row row--{item.status}">
          <Pill status={item.status} />
          <button type="button" class="row__title row__link" title={item.title} onclick={stop(() => openItem(item))} ondblclick={(e) => e.stopPropagation()}>{item.title}</button>
          <Priority pri={item.pri} />
          <DueBadge {item} {today} />
          {#if h && canMarkDone(h.record)}
            <button type="button" class="row__tick" title="Mark done" aria-label="Mark {item.title} done" onclick={stop(() => markDone(h))} ondblclick={(e) => e.stopPropagation()}><Icon name="check" size={12} /></button>
          {/if}
        </li>
      {:else}
        {@const filtered = node.items.length - model.visible.length}
        <li class="row row--empty">{filtered ? `${filtered} hidden by filter` : 'No tasks'}</li>
      {/each}
      {#each node.warnings as w, i (i)}
        <li class="row row--warn" title={w}><Icon name="warning" size={12} /> <span class="row__title">{w}</span></li>
      {/each}
    </ul>
    {#if model.more > 0}
      <button type="button" class="card__more" onclick={stop(onmore)}>+{model.more} more</button>
    {:else if expandedMore}
      <button type="button" class="card__more" onclick={stop(onmore)}>Show less</button>
    {/if}
  {/if}

  {#if roll}
    <div class="card__rollup" title="This node and {roll.nodes} below; {roll.done} done">
      <span><b>{roll.todo}</b> todo</span>
      <span><b>{roll.wip}</b> active</span>
      <span><b>{roll.done}</b> done</span>
      {#if roll.overdue}<span class="card__overdue">{roll.overdue} overdue</span>{/if}
      {#if roll.next}<span class="card__next">next {dueLabel({ id: '', due: roll.next, status: 'todo', title: '', pri: 0 }, today)}</span>{/if}
    </div>
  {/if}
</div>

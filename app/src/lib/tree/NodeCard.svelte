<script lang="ts">
  import type { Item, Node, Source, SourceError } from '../api/types';
  import { MAX_ROWS } from '../constants';
  import { isOpen } from '../fabric/phase';
  import { barAgents, cardTitle, dueLabel, isHidden, isVirtualRoot, rollup, rootCounts, sourceLines, type BoxModel, type Key } from '../model';
  import ErrorText from '../ui/ErrorText.svelte';
  import DueBadge from '../ui/DueBadge.svelte';
  import Icon from '../ui/Icon.svelte';
  import Pill from '../ui/Pill.svelte';
  import Priority from '../ui/Priority.svelte';
  import AddTask from '../fabric/AddTask.svelte';
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
    onsettings?: () => void;
    fabric?: FabricStore | null;
    folder?: string | null;
  }
  let {
    node, tally, model, selected, match, dim, ontoggle, onmore, onopen, onfocus, today = '', source = null, accent = null,
    sources = [], errors = [], pending = [], onsource, onurl, onsettings, fabric = null, folder = null,
  }: Props = $props();

  const hidden = $derived(isHidden(node));
  const virtual = $derived(isVirtualRoot(node));
  const title = $derived(cardTitle(node));
  const barCount = $derived(fabric && folder && node.managed && !virtual ? barAgents(fabric.agentsFor(folder)).length : 0);
  const counts = $derived(virtual && !model.collapsed ? rootCounts(tally ?? node, today) : null);
  const lines = $derived(virtual && !model.collapsed ? sourceLines(tally ?? node, sources, errors, today, undefined, pending) : []);
  const canToggle = $derived(node.children.length > 0 || node.items.length > 0);
  const roll = $derived(model.collapsed ? rollup(tally ?? node, today) : null);
  const expandedMore = $derived(model.shown.length > MAX_ROWS);

  const held = (item: Item) => fabric?.heldFor(folder, node.path, item.id);
  function openItem(item: Item) {
    const h = held(item);
    if (h && fabric) fabric.open(h.path);
  }
  async function markDone(h: Held) {
    try {
      await fabric?.complete(h);
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
    <span class="card__name">{title}</span>
    {#if hidden}
      <span class="card__hidden" title="Hidden node"><Icon name="eye-off" size={12} /></span>
    {/if}
    {#if node.warnings.length}
      <span class="card__warn" title={node.warnings.join('\n')}><Icon name="warning" size={12} /></span>
    {/if}
    {#if onsettings}
      <button type="button" class="chev card__gear" title="Node settings" aria-label="Node settings" onclick={stop(onsettings)} ondblclick={(e) => e.stopPropagation()}>
        <Icon name="gear" size={12} />
      </button>
    {/if}
  </div>

  {#if counts}
    <div class="card__row card__row--counts">
      <span><b>{counts.todo}</b> todo</span><span class="dotsep">·</span>
      <span class:card__wip={counts.active > 0}><b>{counts.active}</b> active</span><span class="dotsep">·</span>
      <span class:card__overdue={counts.overdue > 0}><b>{counts.overdue}</b> overdue</span>
    </div>
  {:else if node.managed && !virtual}
    <div class="card__row card__row--counts">{#if fabric && folder}<TaskStrip {fabric} {folder} />{/if}</div>
  {/if}
  {#if barCount > 0 && fabric}
    <div class="card__row card__row--bars"><TaskStrip {fabric} {folder} part="bars" /></div>
  {/if}

  {#if counts}
    <ul class="card__items">
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
          <AddTask {fabric} {folder} variant="card" />
        </li>
      {/if}
      {#each model.shown as item}
        {@const h = held(item)}
        <li class="row row--{item.phase}">
          <Pill phase={item.phase} />
          <button type="button" class="row__title row__link" title={item.title} onclick={stop(() => openItem(item))} ondblclick={(e) => e.stopPropagation()}>{item.title}</button>
          <Priority pri={item.pri} />
          <DueBadge {item} {today} />
          {#if h && isOpen(item.phase) && !h.record.requests.complete}
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
    <div class="card__rollup" title="This node and {roll.nodes} below; {roll.done} done{roll.suspended ? `, ${roll.suspended} suspended` : ''}">
      <span><b>{roll.todo}</b> todo</span>
      <span><b>{roll.active}</b> active</span>
      <span><b>{roll.done}</b> done</span>
      {#if roll.overdue}<span class="card__overdue">{roll.overdue} overdue</span>{/if}
      {#if roll.next}<span class="card__next">next {dueLabel({ id: '', due: roll.next, phase: 'todo', title: '', pri: 0 }, today)}</span>{/if}
    </div>
  {/if}
</div>

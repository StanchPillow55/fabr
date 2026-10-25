<script lang="ts">
  import { tick } from 'svelte';
  import type { Node, Source, SourceError } from '../api/types';
  import { MAX_ROWS } from '../constants';
  import { cardAside, cardTitle, dueLabel, isHidden, isVirtualRoot, rollup, rootCounts, sourceLines, type BoxModel, type Key } from '../model';
  import ErrorText from '../ui/ErrorText.svelte';
  import DueBadge from '../ui/DueBadge.svelte';
  import Icon from '../ui/Icon.svelte';
  import Pill from '../ui/Pill.svelte';
  import Priority from '../ui/Priority.svelte';
  import TaskStrip from '../fabric/TaskStrip.svelte';
  import type { FabricStore } from '../fabric/tasks.svelte';

  interface Props {
    node: Node;
    tally?: Node;
    model: BoxModel;
    selected: boolean;
    match: boolean;
    dim: boolean;
    ontoggle: () => void;
    oncycle: (index: number) => void;
    onadd?: (title: string) => Promise<boolean>;
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
  }
  let {
    node, tally, model, selected, match, dim, ontoggle, oncycle, onadd, onmore, onopen, onfocus, today = '', source = null, accent = null,
    sources = [], errors = [], pending = [], onsource, onurl, fabric = null, folder = null,
  }: Props = $props();

  const hidden = $derived(isHidden(node));
  const virtual = $derived(isVirtualRoot(node));
  const title = $derived(cardTitle(node));
  const aside = $derived(cardAside(node, source));
  const counts = $derived(virtual && !model.collapsed ? rootCounts(tally ?? node, today) : null);
  const lines = $derived(virtual && !model.collapsed ? sourceLines(tally ?? node, sources, errors, today, undefined, pending) : []);
  const canToggle = $derived(node.children.length > 0 || node.items.length > 0);
  const roll = $derived(model.collapsed ? rollup(tally ?? node, today) : null);
  const expandedMore = $derived(model.shown.length > MAX_ROWS);

  let adding = $state(false);
  let draft = $state('');
  let saving = $state(false);
  let addEl = $state<HTMLInputElement>();
  async function startAdd() {
    adding = true;
    await tick();
    addEl?.focus();
  }
  async function commitAdd(keepOpen: boolean) {
    const text = draft.trim();
    if (saving) return;
    if (!text || !onadd) { draft = ''; adding = false; return; }
    saving = true;
    const ok = await onadd(text);
    saving = false;
    if (ok) draft = '';
    else if (!keepOpen) return;
    if (ok && !keepOpen) adding = false;
    await tick();
    if (adding && keepOpen) addEl?.focus();
  }
  function addKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter') { e.preventDefault(); void commitAdd(true); }
    else if (e.key === 'Escape') { e.preventDefault(); draft = ''; adding = false; }
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
        <span><b>{counts.open}</b> open</span><span class="dotsep">·</span>
        <span class:card__wip={counts.wip > 0}><b>{counts.wip}</b> wip</span><span class="dotsep">·</span>
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
      <li class="row row--add" class:row--adding={adding}>
        {#if adding}
          <span class="row__plus" aria-hidden="true">+</span>
          <input
            class="row__input" type="text" placeholder="Add a todo" aria-label="Add a todo" autocomplete="off" spellcheck="false"
            bind:this={addEl} bind:value={draft} disabled={saving}
            onkeydown={addKey} onblur={() => { if (!saving) void commitAdd(false); }}
            onclick={(e) => e.stopPropagation()} ondblclick={(e) => e.stopPropagation()}
          />
        {:else}
          <button type="button" class="row__addbtn" onclick={stop(startAdd)} ondblclick={(e) => e.stopPropagation()}>
            <span class="row__plus" aria-hidden="true">+</span><span>Add a todo</span>
          </button>
        {/if}
      </li>
      {#each model.shown as item}
        <li class="row row--{item.status}">
          <Pill status={item.status} onclick={() => oncycle(node.items.indexOf(item))} />
          <span class="row__title" title={item.title}>{item.title}</span>
          <Priority pri={item.pri} />
          <DueBadge {item} {today} />
        </li>
      {:else}
        {@const filtered = node.items.length - model.visible.length}
        <li class="row row--empty">{filtered ? `${filtered} hidden by filter` : node.has_todo ? 'No todos' : 'No .todo file'}</li>
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
      <span><b>{roll.wip}</b> wip</span>
      <span><b>{roll.done}</b> done</span>
      {#if roll.overdue}<span class="card__overdue">{roll.overdue} overdue</span>{/if}
      {#if roll.next}<span class="card__next">next {dueLabel({ id: null, due: roll.next, status: 'todo', title: '', pri: null }, today)}</span>{/if}
    </div>
  {/if}
</div>

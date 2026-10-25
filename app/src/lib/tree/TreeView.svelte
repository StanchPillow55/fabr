<script lang="ts">
  import { select } from 'd3-selection';
  import { zoom as d3zoom, zoomIdentity, zoomTransform, type ZoomBehavior, type ZoomTransform } from 'd3-zoom';
  import { untrack } from 'svelte';
  import type { Item, Node } from '../api/types';
  import { layout, type LayoutResult } from '../layout';
  import { boxModel, compareItems, computeForceOpen, matches, nodeKey, type BoxModel, type Key } from '../model';
  import { fitTransform, needsRefit } from '../refit';
  import type { Store } from '../store.svelte';
  import Links from './Links.svelte';
  import NodeCard from './NodeCard.svelte';
  import type { FabricStore } from '../fabric/tasks.svelte';

  let { store, insetTop = 0, fabric = null }: { store: Store; insetTop?: number; fabric?: FabricStore | null } = $props();

  let svgEl = $state<HTMLDivElement>();
  let t = $state<ZoomTransform>(zoomIdentity);
  let zb: ZoomBehavior<HTMLDivElement, unknown> | null = null;
  let raf = 0;
  let dragging = false;
  let animating = false;
  let userMoved = false;

  const view = $derived.by((): { lay: LayoutResult; models: Map<Key, BoxModel> } | null => {
    const root = store.displayRoot;
    if (!root) return null;
    const snap = $state.snapshot(root) as Node;
    const forceOpen = computeForceOpen(store.viewIndex, store.query);
    const ctx = {
      today: store.today, filter: store.filter, hideDone: store.hideDone,
      collapsed: new Set(store.collapsed), moreOpen: new Set(store.moreOpen), forceOpen,
    };
    const models = new Map<Key, BoxModel>();
    const lay = layout(snap, (n) => {
      const m = boxModel({ ...n, items: [...n.items].sort(cardOrder) }, ctx);
      models.set(nodeKey(n), m);
      return m;
    });
    return { lay, models };
  });

  const multi = $derived(store.multiSource);

  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
  function cardOrder(a: Item, b: Item): number {
    return compareItems({ ...a, title: '' }, { ...b, title: '' }) || collator.compare(a.title, b.title);
  }


  const INTERACTIVE = 'input, button, select, textarea, a, label, [contenteditable]';

  $effect(() => {
    const el = svgEl;
    if (!el) return;
    const sel = select(el);
    const z = d3zoom<HTMLDivElement, unknown>()
      .scaleExtent([0.1, 2.5])
      .clickDistance(4)
      .filter((ev: Event) => {
        const target = ev.target as Element | null;
        if (ev.type === 'wheel') return (ev as WheelEvent).ctrlKey || (ev as WheelEvent).metaKey;
        if (target?.closest?.(INTERACTIVE)) return false;
        return !(ev as MouseEvent).button && !(ev as MouseEvent).ctrlKey;
      })
      .on('start', (e: { sourceEvent: Event | null }) => { if (e.sourceEvent) { dragging = true; userMoved = true; } })
      .on('end', () => { dragging = false; })
      .on('zoom', (e: { transform: ZoomTransform }) => { t = e.transform; });
    sel.call(z).on('dblclick.zoom', null);
    const onWheel = (e: WheelEvent) => {
      if (e.ctrlKey || e.metaKey) return;
      e.preventDefault();
      const k = zoomTransform(el).k;
      z.translateBy(sel, -e.deltaX / k, -e.deltaY / k);
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    el.addEventListener('click', onBackground);
    zb = z;
    return () => {
      el.removeEventListener('wheel', onWheel);
      el.removeEventListener('click', onBackground);
      sel.on('.zoom', null);
      zb = null;
    };
  });

  function apply(target: ZoomTransform, ms: number) {
    if (!svgEl || !zb) return;
    cancelAnimationFrame(raf);
    const sel = select(svgEl);
    const z = zb;
    if (ms <= 0 || matchMedia('(prefers-reduced-motion: reduce)').matches) {
      animating = false;
      z.transform(sel, target);
      return;
    }
    animating = true;
    const from = zoomTransform(svgEl);
    const t0 = performance.now();
    const step = (now: number) => {
      const p = Math.min(1, (now - t0) / ms);
      const e = 1 - Math.pow(1 - p, 3);
      const k = from.k + (target.k - from.k) * e;
      const x = from.x + (target.x - from.x) * e;
      const y = from.y + (target.y - from.y) * e;
      z.transform(sel, zoomIdentity.translate(x, y).scale(k));
      if (p < 1) raf = requestAnimationFrame(step);
      else animating = false;
    };
    raf = requestAnimationFrame(step);
  }

  export function fit(ms = 250): void {
    const v = view;
    if (!svgEl || !v || !v.lay.nodes.length) return;
    const { width, height } = svgEl.getBoundingClientRect();
    if (!width || !height) return;
    const f = fitTransform(v.lay, width, height, insetTop);
    userMoved = false;
    apply(zoomIdentity.translate(f.x, f.y).scale(f.k), ms);
  }

  $effect(() => {
    const el = svgEl;
    if (!el || typeof ResizeObserver === 'undefined') return;
    let last = { w: 0, h: 0 };
    const ro = new ResizeObserver((entries) => {
      const r = entries[0]?.contentRect;
      if (!r || !r.width || !r.height) return;
      const changed = Math.abs(r.width - last.w) > 1 || Math.abs(r.height - last.h) > 1;
      last = { w: r.width, h: r.height };
      if (changed && fitted && !userMoved && !dragging) fit(0);
    });
    ro.observe(el);
    return () => ro.disconnect();
  });

  export function checkRefit(): void {
    const v = view;
    if (!svgEl || !v || !v.lay.nodes.length || dragging || animating) return;
    const { width, height } = svgEl.getBoundingClientRect();
    if (!width || !height) return;
    const cur = zoomTransform(svgEl);
    const fitK = fitTransform(v.lay, width, height, insetTop).k;
    const viewport = { x: 0, y: insetTop, w: width, h: Math.max(1, height - insetTop) };
    const bbox = { x: 0, y: 0, w: v.lay.width, h: v.lay.height };
    const cards = v.lay.nodes.map((d) => ({ x: d.x, y: d.y, w: d.w, h: d.h }));
    if (needsRefit(bbox, viewport, cur, fitK, cards)) fit();
  }

  export function centerOn(key: string): void {
    const d = view?.lay.nodes.find((n) => nodeKey(n.node) === key);
    if (!svgEl || !d) return;
    const { width, height } = svgEl.getBoundingClientRect();
    const k = Math.max(zoomTransform(svgEl).k, 0.8);
    const target = zoomIdentity.translate(width / 2 - k * (d.x + d.w / 2), insetTop + (height - insetTop) / 3 - k * d.y).scale(k);
    apply(target, 300);
  }

  export function nextMatch(): void {
    const v = view;
    if (!v || !store.query.trim()) return;
    const hits = v.lay.nodes.filter((d) => matches(d.node, store.query));
    if (!hits.length) return;
    const cur = hits.findIndex((d) => nodeKey(d.node) === store.selected);
    const d = hits[(cur + 1) % hits.length];
    store.selected = nodeKey(d.node);
    centerOn(store.selected);
  }

  let fitted = false;
  let lastFocus: Key | null | undefined = undefined;
  let pending = 0;
  let pendingFit = false;
  function schedule(force: boolean) {
    pendingFit ||= force;
    if (pending) return;
    pending = requestAnimationFrame(() => {
      pending = 0;
      const f = pendingFit;
      pendingFit = false;
      if (f) fit();
      else checkRefit();
    });
  }
  $effect(() => {
    const v = view;
    const f = store.focus;
    if (!v || !svgEl) return;
    untrack(() => {
      if (!fitted) {
        fitted = true;
        lastFocus = f;
        requestAnimationFrame(() => fit(0));
      } else if (f !== lastFocus) {
        lastFocus = f;
        schedule(true);
      }
    });
  });
  $effect(() => {
    void store.layoutVersion;
    void store.query;
    untrack(() => {
      if (fitted) schedule(false);
    });
  });
  $effect(() => () => cancelAnimationFrame(pending));

  function onBackground(e: MouseEvent) {
    if (e.target === svgEl) store.selected = null;
  }
</script>

<div bind:this={svgEl} class="tree" role="application" aria-label="Workspace tree">
  {#if view}
    <svg class="tree__links" aria-hidden="true">
      <g transform="translate({t.x},{t.y}) scale({t.k})">
        <Links links={view.lay.links} colors={store.colors} />
      </g>
    </svg>
    <div class="cards" style="transform: translate({t.x}px, {t.y}px) scale({t.k})">
      {#each view.lay.nodes as d (nodeKey(d.node))}
        {@const k = nodeKey(d.node)}
        {@const m = view.models.get(k)!}
        <div class="cardpos" style="left:{d.x}px;top:{d.y}px;width:{d.w}px;height:{d.h}px">
          <NodeCard
            node={d.node}
            tally={store.tallyNode(d.node)}
            model={m}
            today={store.today}
            source={multi && d.node.source && d.node.path === '' ? (store.sourceOf(d.node.source) ?? null) : null}
            accent={store.colors.get(k) ?? null}
            selected={store.selected === k}
            match={matches(d.node, store.query)}
            dim={d.node.managed && store.filter !== 'all' && !m.visible.length}
            ontoggle={() => store.toggleCollapse(k)}
            onmore={() => store.toggleMore(k)}
            onopen={() => store.activate(k)}
            onfocus={() => store.setFocus(k)}
            sources={store.tree?.sources ?? []}
            errors={store.tree?.errors ?? []}
            pending={store.tree?.pending ?? []}
            onsource={(sk) => store.openNodeSettings(sk)}
            onurl={(u) => store.openUrl(u)}
            onsettings={() => store.openNodeSettings(k)}
            {fabric}
            remote={!!store.sourceOf(d.node.source)?.host}
            folder={fabric ? fabric.folderFor(store.sourceOf(d.node.source), d.node.path) : null}
          />
        </div>
      {/each}
    </div>
  {/if}
</div>

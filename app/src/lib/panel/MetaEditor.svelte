<script lang="ts">
  import type { MetaValue } from '../api/types';
  import { COLOR_NAMES, COLOR_RE, colorCss } from '../model';
  import Icon from '../ui/Icon.svelte';
  import { commitInput } from '../ui/actions';

  let { entries, known, note = null, oncommit, onerror, inherited = null, section }: {
    section: 'identity' | 'display' | 'more';
    entries: [string, MetaValue][];
    known: Record<string, MetaValue>;
    inherited?: string | null;
    note?: { key: string; kind: 'ok' | 'error'; text: string } | null;
    oncommit: (key: string, value: MetaValue | undefined) => void;
    onerror?: (msg: string | null) => void;
  } = $props();

  const ownRaw = $derived(entries.find(([k]) => k === 'color')?.[1]);
  const own = $derived(typeof ownRaw === 'string' && COLOR_RE.test(ownRaw) ? ownRaw : null);
  const current = $derived(own ?? inherited);
  const row = (k: string) => entries.find(([x]) => x === k);
  const rowsOf = (keys: string[]) => keys.map(row).filter((r): r is [string, MetaValue] => !!r);
  const LABELS: Record<string, string> = { show_depth: 'Show folders', hidden: 'Hidden' };
  const HINTS: Record<string, string> = { show_depth: 'how many levels of plain folders to draw below this node' };
  const otherRows = $derived(entries.filter(([k]) => !(k in known) && (inherited === null || k !== 'color')));

  function pickColor(c: string | null) {
    onerror?.(null);
    if (c !== null && c === own) return;
    if (c === null && ownRaw === undefined) return;
    oncommit('color', c ?? undefined);
  }
  function commitHex(el: HTMLInputElement) {
    let v = el.value.trim();
    if (!v) return;
    if (/^[0-9A-Fa-f]{6}$/.test(v)) v = `#${v}`;
    if (!/^#[0-9A-Fa-f]{6}$/.test(v)) {
      onerror?.('Colour: use #RRGGBB, e.g. #3a7bd5.');
      return;
    }
    el.value = v;
    if (v.toLowerCase() !== own?.toLowerCase()) pickColor(v);
  }

  let newKey = $state('');
  let newVal = $state('');

  function save(k: string, v: MetaValue, el: HTMLInputElement) {
    const next: MetaValue = typeof v === 'number' ? (el.value === '' ? 0 : Math.max(0, Math.floor(+el.value))) : el.value;
    if (next === v) {
      if (typeof v === 'number') el.value = String(v);
      return;
    }
    oncommit(k, next);
  }
  function addKey() {
    const k = newKey.trim();
    if (!/^[A-Za-z0-9_-]+$/.test(k)) return onerror?.('Keys may use letters, digits, _ and - only.');
    if (entries.some(([x]) => x === k)) return onerror?.(`Key ${k} already exists.`);
    onerror?.(null);
    oncommit(k, newVal);
    newKey = '';
    newVal = '';
  }
</script>

{#snippet field(k: string, v: MetaValue)}
  <div class="mrow">
    <label class="mrow__key" for="meta-{k}" style:text-transform={LABELS[k] ? 'none' : undefined}>{LABELS[k] ?? k.replace('_', ' ')}</label>
    {#if typeof v === 'boolean'}
      <span class="mrow__val">
        <input id="meta-{k}" class="switch" type="checkbox" role="switch" checked={v} onchange={(e) => oncommit(k, e.currentTarget.checked)} />
      </span>
    {:else if typeof v === 'number'}
      <input id="meta-{k}" class="mrow__val mrow__input" type="number" min="0" step="1" value={v}
        {@attach commitInput((el) => save(k, v, el))} />
    {:else}
      <input id="meta-{k}" class="mrow__val mrow__input" type="text" value={v} placeholder={k === 'description' ? 'Optional' : ''}
        {@attach commitInput((el) => save(k, v, el))} />
    {/if}
    {#if note?.key === k}
      <span class="mrow__note" class:mrow__note--error={note.kind === 'error'} role="status">{note.text}</span>
    {/if}
    {#if k in known}
      <span class="mrow__spacer"></span>
      {#if HINTS[k]}<span class="mrow__hint">{HINTS[k]}</span>{/if}
    {:else}
      <button type="button" class="btn btn--icon btn--danger" title="Remove key" aria-label="Remove {k}" onclick={() => oncommit(k, undefined)}><Icon name="x" size={12} /></button>
    {/if}
  </div>
{/snippet}

<div class="group">
  {#if section === 'identity'}
  {#each rowsOf(['name', 'kind']) as [k, v] (k)}{@render field(k, v)}{/each}
  {#if inherited !== null}
    <div class="mrow mrow--color">
      <span class="mrow__key" id="meta-color-label">Colour</span>
      <div class="mrow__colors">
        <div class="swatches swatches--sm" role="radiogroup" aria-labelledby="meta-color-label">
          {#each COLOR_NAMES as c (c)}
            {@const on = current === c}
            <button type="button" role="radio" aria-checked={on} aria-label={c} title={on && !own ? `${c} (inherited)` : c}
              class="swatch" class:swatch--on={on && !!own} class:swatch--inherited={on && !own} style:--swatch="var(--{c})"
              onclick={() => pickColor(c)}>
              {#if on}<Icon name="check" size={10} />{/if}
            </button>
          {/each}
        </div>
        <div class="mrow__colorline">
          {#if current?.startsWith('#')}
            <span class="swatch swatch--hex" class:swatch--on={!!own} class:swatch--inherited={!own} style:--swatch={colorCss(current)} aria-hidden="true"></span>
          {/if}
          <input class="mrow__input mrow__hex" type="text" maxlength="7" spellcheck="false" placeholder="#RRGGBB" aria-label="Colour hex"
            value={own?.startsWith('#') ? own : ''} {@attach commitInput(commitHex)} />
          <button type="button" class="btn btn--tint mrow__inherit" class:mrow__inherit--on={!own} aria-pressed={!own} onclick={() => pickColor(null)}>Inherit</button>
          {#if note?.key === 'color'}
            <span class="mrow__note" class:mrow__note--error={note.kind === 'error'} role="status">{note.text}</span>
          {:else if !own}
            <span class="mrow__note">{typeof ownRaw === 'string' && ownRaw ? `invalid “${ownRaw}”, inherited` : 'inherited'}</span>
          {/if}
        </div>
      </div>
    </div>
  {/if}
  {#each rowsOf(['description']) as [k, v] (k)}{@render field(k, v)}{/each}
  {:else if section === 'display'}
  {#each rowsOf(['show_depth', 'hidden']) as [k, v] (k)}{@render field(k, v)}{/each}
  {:else}
  {#each otherRows as [k, v] (k)}{@render field(k, v)}{/each}
  <form class="mrow mrow--add" onsubmit={(e) => { e.preventDefault(); addKey(); }}>
    <input class="mrow__key mrow__input" type="text" placeholder="New key" aria-label="New key" bind:value={newKey} />
    <input class="mrow__val mrow__input" type="text" placeholder="Value" aria-label="New value" bind:value={newVal} />
    <button type="submit" class="btn btn--tint" disabled={!newKey.trim()}>Add</button>
  </form>
  {/if}
</div>

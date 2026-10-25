import { describe, expect, it } from 'vitest';
import { BARS, COUNTS, HEAD, LIST_PAD, ROLLUP, ROW } from './constants';

import { readFileSync } from 'node:fs';
const read = (p: string) => readFileSync(new URL(p, import.meta.url), 'utf8');
const css = read('../styles/components.css');
const treeView = read('./tree/TreeView.svelte');

const px = (re: RegExp): number => {
  const m = re.exec(css);
  if (!m) throw new Error(`css rule not found: ${re}`);
  return Number(m[1]);
};

describe('card overlay (no foreignObject)', () => {
  it('the CSS box sizes agree with the layout constants (boxModel heights)', () => {
    expect(px(/\.card__head \{[^}]*?height: (\d+)px/s)).toBe(HEAD);
    expect(px(/\.card__row--counts \{[^}]*?height: (\d+)px/)).toBe(COUNTS);
    expect(px(/\.card__row--bars \{[^}]*?height: (\d+)px/)).toBe(BARS);
    expect(px(/\.row \{[^}]*?height: (\d+)px/)).toBe(ROW);
    expect(px(/\.card__rollup \{[^}]*?height: (\d+)px/)).toBe(ROLLUP);
    expect(px(/\.card__more \{[^}]*?height: (\d+)px/)).toBe(ROW);
    expect(css).not.toMatch(/\.row--add[^{]*\{[^}]*height:/);
    expect(px(/\.card__items \{[^}]*?padding: (\d+)px/)).toBe(LIST_PAD / 2);
  });

  it('no foreignObject anywhere in the tree view or styles', () => {
    expect(treeView).not.toMatch(/<foreignObject/);
    expect(css).not.toMatch(/foreignObject/);
  });

  it('cards carry no filter / transform / will-change effect', () => {
    const cardRules = css.split('}').filter((r: string) => /\.(card|pill|row|chev|pri|due)\b/.test(r.split('{')[0] ?? '') && !/^\s*@/.test(r));
    for (const r of cardRules) expect(r.split('{')[1] ?? '').not.toMatch(/(^|[;\s])(filter|transform|will-change|opacity)\s*:/);
  });

  it('the overlay is placed with the same x/y/w/h the links use', () => {
    const src = treeView;
    expect(src).toContain('left:{d.x}px;top:{d.y}px;width:{d.w}px;height:{d.h}px');
    expect(src).toContain('translate({t.x}px, {t.y}px) scale({t.k})');
    expect(src).toContain('translate({t.x},{t.y}) scale({t.k})');
  });
});

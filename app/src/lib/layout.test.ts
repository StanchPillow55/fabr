import { describe, expect, it } from 'vitest';
import type { Node } from './api/types';
import { GAP_X, LEVEL_GAP, W } from './constants';
import { layout, linkPath } from './layout';
import { boxModel, nodeKey } from './model';

const node = (path: string, o: Partial<Node> = {}): Node => ({
  source: 'local', path, name: path.split('/').pop() || 'root', managed: true, meta: {}, items: [],
  warnings: [], version: 'v', via: null, has_todo: true, children: [], ...o,
});

const H: Record<string, number> = { '': 40, a: 100, b: 60, 'a/x': 30, 'a/y': 80, 'b/z': 50 };
const tree = () =>
  node('', { children: [node('a', { children: [node('a/x'), node('a/y')] }), node('b', { children: [node('b/z')] })] });

describe('layout', () => {
  it('stacks levels by the tallest card of the level above', () => {
    const r = layout(tree(), (n) => ({ h: H[n.path] }));
    const y = Object.fromEntries(r.nodes.map((d) => [d.node.path, d.y]));
    expect(y['']).toBe(0);
    expect(y.a).toBe(40 + LEVEL_GAP);
    expect(y.b).toBe(40 + LEVEL_GAP);
    expect(y['a/x']).toBe(40 + LEVEL_GAP + 100 + LEVEL_GAP);
    expect(y['b/z']).toBe(y['a/x']);
    expect(r.height).toBe(y['a/y'] + 80);
  });

  it('places siblings at least one column apart and starts at x = 0', () => {
    const r = layout(tree(), (n) => ({ h: H[n.path] }));
    const xs = r.nodes.map((d) => d.x);
    expect(Math.min(...xs)).toBe(0);
    const level2 = r.nodes.filter((d) => d.depth === 2).map((d) => d.x).sort((a, b) => a - b);
    for (let i = 1; i < level2.length; i++) expect(level2[i] - level2[i - 1]).toBeGreaterThanOrEqual(W + GAP_X - 1e-6);
    expect(r.nodes.every((d) => d.w === W)).toBe(true);
    expect(r.width).toBe(Math.max(...xs) + W);
  });

  it('excludes the subtree of a collapsed node but keeps its card', () => {
    const r = layout(tree(), (n) => ({ h: H[n.path], collapsed: n.path === 'a' }));
    const paths = r.nodes.map((d) => d.node.path).sort();
    expect(paths).toEqual(['', 'a', 'b', 'b/z']);
    expect(r.links.map((l) => `${l.from.node.path}>${l.to.node.path}`).sort()).toEqual(['>a', '>b', 'b>b/z']);
    expect(r.nodes.find((d) => d.node.path === 'b/z')!.y).toBe(40 + LEVEL_GAP + 100 + LEVEL_GAP);
  });

  it('works with boxModel: a collapsed node\'s children are not laid out', () => {
    const root = node('', { children: [node('arch', { items: [{ id: null, status: 'done', due: null, title: 'x', pri: null }], children: [node('arch/old')] })] });
    const none = new Set<string>();
    const ctx = { today: '2026-10-19', filter: 'all' as const, hideDone: false, collapsed: new Set(['local:arch']), moreOpen: none, forceOpen: none };
    const r = layout(root, (n) => boxModel(n, ctx));
    expect(r.nodes.map((d) => nodeKey(d.node))).toEqual(['local:', 'local:arch']);
    const r2 = layout(root, (n) => boxModel(n, { ...ctx, collapsed: none }));
    expect(r2.nodes).toHaveLength(3);
  });

  it('linkPath draws an orthogonal connector', () => {
    const r = layout(tree(), (n) => ({ h: H[n.path] }));
    const l = r.links[0];
    const d = linkPath(l.from, l.to);
    expect(d.startsWith(`M${l.from.x + W / 2},${l.from.y + l.from.h}`)).toBe(true);
    expect(d.endsWith(`V${l.to.y}`)).toBe(true);
  });
});

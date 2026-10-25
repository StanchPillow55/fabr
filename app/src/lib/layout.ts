import { hierarchy, tree as d3tree } from 'd3-hierarchy';
import type { Node } from './api/types';
import { GAP_X, LEVEL_GAP, W } from './constants';

export interface LaidOut {
  node: Node;
  x: number;
  y: number;
  w: number;
  h: number;
  depth: number;
}

export interface LayoutResult {
  nodes: LaidOut[];
  links: { from: LaidOut; to: LaidOut }[];
  width: number;
  height: number;
}

export interface Box {
  h: number;
  collapsed?: boolean;
}

export function layout(root: Node, boxFor: (n: Node) => Box): LayoutResult {
  const boxes = new Map<Node, Box>();
  const box = (n: Node) => {
    let b = boxes.get(n);
    if (!b) boxes.set(n, (b = boxFor(n)));
    return b;
  };
  const h = hierarchy(root, (n) => (box(n).collapsed ? null : n.children));
  const laid = d3tree<Node>()
    .nodeSize([W + GAP_X, 1])
    .separation((a, b) => (a.parent === b.parent ? 1 : 1.15))(h);

  const levelH: number[] = [];
  laid.each((d) => { levelH[d.depth] = Math.max(levelH[d.depth] ?? 0, box(d.data).h); });
  const levelY = [0];
  for (let i = 1; i < levelH.length; i++) levelY[i] = levelY[i - 1] + levelH[i - 1] + LEVEL_GAP;

  let minX = Infinity;
  laid.each((d) => { minX = Math.min(minX, d.x); });
  const byNode = new Map<object, LaidOut>();
  const nodes: LaidOut[] = [];
  let maxX = 0;
  let maxY = 0;
  laid.each((d) => {
    const o: LaidOut = { node: d.data, x: d.x - minX, y: levelY[d.depth], w: W, h: box(d.data).h, depth: d.depth };
    byNode.set(d, o);
    nodes.push(o);
    maxX = Math.max(maxX, o.x + W);
    maxY = Math.max(maxY, o.y + o.h);
  });
  const links = laid.links().map((l) => ({ from: byNode.get(l.source)!, to: byNode.get(l.target)! }));
  return { nodes, links, width: maxX, height: maxY };
}

export function linkPath(from: LaidOut, to: LaidOut, r = 6): string {
  const x1 = from.x + from.w / 2;
  const y1 = from.y + from.h;
  const x2 = to.x + to.w / 2;
  const y2 = to.y;
  const mid = y2 - LEVEL_GAP / 2;
  const dx = x2 - x1;
  if (Math.abs(dx) < 0.5) return `M${x1},${y1}V${y2}`;
  const rr = Math.min(r, Math.abs(dx) / 2, (mid - y1) / 2, (y2 - mid) / 2);
  const s = Math.sign(dx);
  return `M${x1},${y1}V${mid - rr}Q${x1},${mid} ${x1 + s * rr},${mid}H${x2 - s * rr}Q${x2},${mid} ${x2},${mid + rr}V${y2}`;
}

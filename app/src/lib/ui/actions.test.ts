import { describe, expect, it } from 'vitest';
import { commit } from './actions';

type Handler = (e: unknown) => void;

function fakeNode() {
  const handlers = new Map<string, Set<Handler>>();
  const node = {
    blurred: 0,
    addEventListener(type: string, h: Handler) {
      if (!handlers.has(type)) handlers.set(type, new Set());
      handlers.get(type)!.add(h);
    },
    removeEventListener(type: string, h: Handler) {
      handlers.get(type)?.delete(h);
    },
    blur() {
      node.blurred++;
      node.fire('blur', {});
    },
    fire(type: string, e: unknown) {
      for (const h of handlers.get(type) ?? []) h(e);
    },
    count: () => [...handlers.values()].reduce((a, s) => a + s.size, 0),
  };
  return node;
}

function key(k: string) {
  const e = { key: k, prevented: false, stopped: false, preventDefault() { e.prevented = true; }, stopPropagation() { e.stopped = true; } };
  return e;
}

describe('commit action', () => {
  it('commits on Enter and on blur, and detaches on destroy', () => {
    const node = fakeNode();
    let n = 0;
    const a = commit(node as unknown as HTMLElement, () => n++);
    const e = key('Enter');
    node.fire('keydown', e);
    expect([n, e.prevented, e.stopped]).toEqual([1, true, true]);
    const other = key('a');
    node.fire('keydown', other);
    expect([n, other.prevented, other.stopped]).toEqual([1, false, false]);
    node.fire('blur', {});
    expect(n).toBe(2);
    a.destroy();
    expect(node.count()).toBe(0);
    node.fire('blur', {});
    expect(n).toBe(2);
  });

  it('enter: blur blurs the node and commits once through blur', () => {
    const node = fakeNode();
    let n = 0;
    const a = commit(node as unknown as HTMLElement, { fn: () => n++, enter: 'blur' });
    const e = key('Enter');
    node.fire('keydown', e);
    expect([n, node.blurred, e.prevented, e.stopped]).toEqual([1, 1, true, true]);
    let m = 0;
    a.update(() => m++);
    node.fire('keydown', key('Enter'));
    expect([n, m, node.blurred]).toEqual([1, 1, 1]);
  });
});

import { describe, expect, it } from 'vitest';
import { fitTransform, needsRefit } from './refit';

const vp = { x: 0, y: 50, w: 1000, h: 650 };
const bbox = { x: 0, y: 0, w: 800, h: 400 };

describe('needsRefit', () => {
  it('all off-screen -> true', () => {
    expect(needsRefit(bbox, vp, { x: -2000, y: 100, k: 1 }, 1)).toBe(true);
    expect(needsRefit(bbox, vp, { x: 100, y: 900, k: 1 }, 1)).toBe(true);
    expect(needsRefit(bbox, vp, { x: 100, y: -360, k: 0.1 }, 1)).toBe(true);
  });

  it('partially visible -> false', () => {
    expect(needsRefit(bbox, vp, { x: -700, y: 100, k: 1 }, 1)).toBe(false);
    expect(needsRefit(bbox, vp, { x: 600, y: 500, k: 1 }, 1)).toBe(false);
  });

  it('tiny (scale ratio < 0.6, fully inside) -> true', () => {
    expect(needsRefit(bbox, vp, { x: 100, y: 100, k: 0.5 }, 1.1)).toBe(true);
    expect(needsRefit(bbox, vp, { x: 100, y: 100, k: 0.6 }, 1)).toBe(true);
  });

  it('normal -> false', () => {
    expect(needsRefit(bbox, vp, { x: 100, y: 100, k: 1 }, 1.1)).toBe(false);
    expect(needsRefit(bbox, vp, { x: 100, y: 100, k: 0.7 }, 1)).toBe(false);
    expect(needsRefit(bbox, vp, { x: -300, y: 100, k: 0.5 }, 1.1)).toBe(false);
  });

  it('cards: a bbox overlapping the viewport with no card in it is off-screen', () => {
    const cards = [{ x: 0, y: 0, w: 100, h: 50 }, { x: 700, y: 350, w: 100, h: 50 }];
    const t = { x: -250, y: -100, k: 1 };
    expect(needsRefit(bbox, { x: 0, y: 0, w: 300, h: 300 }, t, 1, cards)).toBe(true);
    expect(needsRefit(bbox, { x: 0, y: 0, w: 300, h: 300 }, t, 1)).toBe(false);
  });
});

describe('fitTransform', () => {
  it('centres and caps the scale at 1.1', () => {
    const t = fitTransform({ width: 100, height: 100 }, 1000, 700, 50);
    expect(t.k).toBe(1.1);
    expect(t.x).toBeCloseTo(500 - 55);
  });
  it('shrinks big trees, never below 0.1', () => {
    expect(fitTransform({ width: 1804, height: 400 }, 1000, 700).k).toBeCloseTo(0.5);
    expect(fitTransform({ width: 1e6, height: 400 }, 1000, 700).k).toBe(0.1);
  });
});

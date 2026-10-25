
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}
export interface Transform {
  x: number;
  y: number;
  k: number;
}

export const TINY_RATIO = 0.6;
const PAD = 48;

export function fitTransform(lay: { width: number; height: number }, width: number, height: number, insetTop = 0): Transform {
  const avail = Math.max(100, height - insetTop);
  const k = Math.max(0.1, Math.min((width - PAD * 2) / lay.width, (avail - PAD * 2) / lay.height, 1.1));
  return { x: width / 2 - (k * lay.width) / 2, y: insetTop + Math.max(PAD, avail / 2 - (k * lay.height) / 2), k };
}

const toScreen = (r: Rect, t: Transform): Rect => ({ x: t.x + t.k * r.x, y: t.y + t.k * r.y, w: t.k * r.w, h: t.k * r.h });
const intersects = (a: Rect, b: Rect): boolean => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
const inside = (a: Rect, b: Rect): boolean => a.x >= b.x && a.y >= b.y && a.x + a.w <= b.x + b.w && a.y + a.h <= b.y + b.h;

export function needsRefit(bbox: Rect, viewport: Rect, transform: Transform, fitScale: number, cards?: Rect[]): boolean {
  const boxes = cards?.length ? cards : [bbox];
  if (!boxes.some((c) => intersects(toScreen(c, transform), viewport))) return true;
  return inside(toScreen(bbox, transform), viewport) && transform.k <= TINY_RATIO * fitScale;
}

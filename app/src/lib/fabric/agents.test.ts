import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import AgentsScreen from './AgentsScreen.svelte';
import { ageSeconds, elapsedSeconds, isDead, parseRegistryEntry, sortEntries, type RegistryEntry } from './agents';

const NOW = '2026-10-24T19:00:00Z';
const raw = import.meta.glob('../../../../fixtures/{05,06,10}-*/home/.fabric-space/registry/*.yaml', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;
const entriesOf = (fx: string) => Object.keys(raw).filter((k) => k.includes(`/${fx}`)).sort().map((k) => parseRegistryEntry(raw[k]));

describe('parseRegistryEntry', () => {
  it('parses all five fixture files', () => {
    expect(Object.keys(raw)).toHaveLength(5);
    for (const t of Object.values(raw)) {
      const e = parseRegistryEntry(t);
      expect(e.task_path.endsWith('.md')).toBe(true);
      expect(e.heartbeat_interval_s).toBe(10);
      expect(Array.isArray(e.log_tail)).toBe(true);
    }
  });
  it('keeps the question of the waiting fixture', () => {
    expect(entriesOf('06')[0].question?.id).toBe('q3');
    expect(entriesOf('05')[0].question).toBeNull();
  });
  it('throws naming the field', () => {
    const good = Object.values(raw)[0];
    expect(() => parseRegistryEntry(good.replace(/^pid:.*$/m, 'pid: abc'))).toThrow(/pid/);
    expect(() => parseRegistryEntry(good.replace(/^heartbeat_at:.*$/m, 'heartbeat_at: yesterday'))).toThrow(/heartbeat_at/);
    expect(() => parseRegistryEntry(good.replace(/^task_id:.*$/m, ''))).toThrow(/task_id/);
  });
});

describe('time maths', () => {
  const f10 = entriesOf('10');
  const by = (id: string) => f10.find((e) => e.task_id.endsWith(id))!;
  it('fixture 10 has two live and one dead at the fixed now', () => {
    expect(f10).toHaveLength(3);
    expect(f10.filter((e) => !isDead(e, NOW)).map((e) => e.task_id.slice(11)).sort()).toEqual(['file-tax-receipts', 'order-filament']);
    expect(isDead(by('rename-scans'), NOW)).toBe(true);
  });
  it('flips when now passes three intervals', () => {
    const e = by('order-filament');
    const hb = Date.parse(e.heartbeat_at);
    expect(isDead(e, new Date(hb + 30_000))).toBe(false);
    expect(isDead(e, new Date(hb + 30_001))).toBe(true);
  });
  it('computes age and elapsed', () => {
    const e = by('rename-scans');
    expect(ageSeconds(e, NOW)).toBe(150);
    expect(elapsedSeconds(e, new Date(NOW))).toBe(720);
  });
});

describe('sortEntries', () => {
  it('puts newest started first without mutating', () => {
    const f10 = entriesOf('10');
    const s = sortEntries(f10);
    const t = s.map((e) => Date.parse(e.started_at));
    expect(t).toEqual([...t].sort((a, b) => b - a));
    expect(s).not.toBe(f10);
  });
});

describe('AgentsScreen', () => {
  const rows = (html: string) => html.split('data-testid="agent-row"').length - 1;
  const count = (html: string, s: string) => html.split(s).length - 1;
  it('renders three rows, one Dead mark, two Cancel buttons', () => {
    const { body } = render(AgentsScreen, { props: { entries: entriesOf('10'), now: NOW } });
    expect(rows(body)).toBe(3);
    expect(count(body, 'arow__dead')).toBe(1);
    expect(count(body, '>Cancel<')).toBe(2);
    expect(body).toContain('The backend removes stale entries on start.');
  });
  it('shows titles with fallback and the pending question', () => {
    const [e] = entriesOf('06');
    const { body } = render(AgentsScreen, { props: { entries: [e], now: NOW, titleFor: () => 'Order filament' } });
    expect(body).toContain('Order filament');
    expect(body).toContain('May I fill it for this task?');
    expect(render(AgentsScreen, { props: { entries: [e], now: NOW } }).body).toContain(e.task_id);
  });
  it('collapses the log to the last three lines', () => {
    const [e] = entriesOf('05');
    const { body } = render(AgentsScreen, { props: { entries: [e], now: NOW } });
    expect(body).not.toContain('reached the checkout form');
    expect(body).toContain('selected standard shipping');
  });
  it('shows the empty state', () => {
    const { body } = render(AgentsScreen, { props: { entries: [] as RegistryEntry[], now: NOW } });
    expect(rows(body)).toBe(0);
    expect(body).toContain('No agents running');
  });
});

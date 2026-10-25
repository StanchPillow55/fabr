import { describe, expect, it } from 'vitest';
import { buildSource, isUntouched, parseDepth, parsePrune, rowError, sourceStatus, suggestId, uniqueId, validateDraft } from './drafts';

const src = (o = {}) => ({ id: 'a', name: 'A', host: null, root: '/x', prune: null, ...o });

describe('draft helpers', () => {
  it('parses prune and depth', () => {
    expect(parsePrune('a  b,c')).toEqual(['a', 'b', 'c']);
    expect(parsePrune('  ')).toBeNull();
    expect(parseDepth('')).toEqual({ value: null });
    expect(parseDepth('6')).toEqual({ value: 6 });
    expect(parseDepth('0').error).toBeTruthy();
    expect(parseDepth('13').error).toBeTruthy();
    expect(parseDepth('2.5').error).toBeTruthy();
  });
  it('ids', () => {
    expect(uniqueId('My Box', [])).toBe('my-box');
    expect(uniqueId('my-box', ['my-box', 'my-box-2'])).toBe('my-box-3');
    expect(suggestId({ name: '', host: null, root: '/home/me/Work/' }, [])).toBe('work');
    expect(suggestId({ name: '', host: 'devbox', root: '~/w' }, ['devbox'])).toBe('devbox-2');
  });
  it('never yields an empty id', () => {
    for (const name of ['ÄÖ', '🚀', '   ', '---', '日本語']) {
      expect(suggestId({ name, host: null, root: '' }, [])).toMatch(/^[a-z0-9_-]+$/);
    }
    expect(suggestId({ name: '🚀', host: null, root: '' }, ['source'])).toBe('source-2');
    expect(uniqueId('', [])).toBe('source');
    expect(rowError('Box', 'x')).toBe('“Box”: x');
  });
  it('validates', () => {
    expect(validateDraft(src(), '', [])).toEqual({});
    expect(validateDraft(src({ root: ' ' }), '', []).root).toBeTruthy();
    expect(validateDraft(src({ id: 'Bad Id' }), '', []).id).toBeTruthy();
    expect(validateDraft(src(), '', ['a']).id).toBeTruthy();
    expect(validateDraft(src(), '20', []).depth).toBeTruthy();
  });
  it('builds the saved source', () => {
    expect(buildSource(src({ name: ' ', host: ' ', root: ' /x ' }), 'vendor tmp', '3'))
      .toEqual({ id: 'a', name: 'a', host: null, root: '/x', prune: ['vendor', 'tmp'], search_depth: 3 });
    expect(buildSource(src(), '', '')).not.toHaveProperty('search_depth');
    expect(isUntouched(src({ name: '', root: '' }), '', '')).toBe(true);
    expect(isUntouched(src({ name: '', root: 'x' }), '', '')).toBe(false);
  });
  it('status dot', () => {
    const ok = { ok: true, message: '', latency_ms: 1, node_count: 1 };
    const bad = { ...ok, ok: false };
    expect(sourceStatus('a', ok, [{ source: 'a', message: 'x' }], ['a'])).toBe('ok');
    expect(sourceStatus('a', bad, [], ['a'])).toBe('error');
    expect(sourceStatus('a', null, [{ source: 'a', message: 'x' }], ['a'])).toBe('error');
    expect(sourceStatus('a', null, [{ source: 'b', message: 'x' }], ['a'])).toBe('ok');
    expect(sourceStatus('n', null, [], ['a'])).toBe('untested');
  });
});

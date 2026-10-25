import { describe, expect, it } from 'vitest';
import { pickLocations } from './live';

describe('pickLocations', () => {
  it('prefers the environment and falls back to the home directory', () => {
    expect(pickLocations({ root: '/r', home: '/h' }, '/u')).toEqual({ roots: ['/r'], home: '/h' });
    expect(pickLocations({}, '/u')).toEqual({ roots: ['/u'], home: '/u' });
  });
  it('refuses to guess in a plain browser', () => {
    expect(() => pickLocations({ root: '/r' }, null)).toThrow('Task folders are not set up yet. Finish setup to choose your default folder.');
  });
});

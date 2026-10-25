import { describe, expect, it } from 'vitest';
import { pickLocations } from './live';
import { emptySettings } from '../setup/setup-source';

const withFolder = (f: string) => ({ exists: true, settings: { ...emptySettings(), default_folder: f } });

describe('pickLocations', () => {
  it('uses the default folder from settings', () => {
    expect(pickLocations({}, '/u', withFolder('/home/me/3d'))).toEqual({ roots: ['/home/me/3d'], home: '/u' });
  });
  it('lets the environment overrides win', () => {
    expect(pickLocations({ root: '/r', home: '/h' }, '/u', withFolder('/home/me/3d'))).toEqual({ roots: ['/r'], home: '/h' });
  });
  it('derives home from the folder only when nothing else knows it', () => {
    expect(pickLocations({}, null, withFolder('/home/me/3d'))).toEqual({ roots: ['/home/me/3d'], home: '/home/me/3d' });
  });
  it('refuses to guess without settings or overrides', () => {
    const msg = 'Task folders are not set up yet. Finish setup to choose your default folder.';
    expect(() => pickLocations({}, null)).toThrow(msg);
    expect(() => pickLocations({}, '/u', { exists: false, settings: emptySettings() })).toThrow(msg);
    expect(() => pickLocations({ root: '/r' }, null)).toThrow(msg);
  });
});

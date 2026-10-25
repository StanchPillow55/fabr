import { createApi } from '../api/index';
import { isTauri } from '../api/env';
import { FixtureSource, type FabricSource } from './source';
import { TauriSource } from './tauri-source';

export const FOLDERS_UNKNOWN_USER = 'Task folders are not set up yet. Finish setup to choose your default folder.';

export interface LiveLocations { roots: string[]; home: string }

export function pickLocations(env: { root?: string; home?: string }, homeDir: string | null): LiveLocations {
  const home = env.home || homeDir || '';
  const root = env.root || homeDir || '';
  if (!home || !root) {
    if (import.meta.env.DEV) console.warn('fabric folders unknown: set VITE_FABRIC_ROOT and VITE_FABRIC_HOME, or run inside the desktop app');
    throw new Error(FOLDERS_UNKNOWN_USER);
  }
  return { roots: [root], home };
}

export async function createFabricSource(): Promise<FabricSource> {
  if (!isTauri() && import.meta.env.VITE_MOCK === '1') return new FixtureSource();
  let homeDir: string | null = null;
  if (isTauri()) homeDir = (await (await import('@tauri-apps/api/path')).homeDir()).replace(/\/+$/, '');
  const { roots, home } = pickLocations({ root: import.meta.env.VITE_FABRIC_ROOT, home: import.meta.env.VITE_FABRIC_HOME }, homeDir);
  return new TauriSource(await createApi(), roots, home);
}

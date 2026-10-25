import { isTauri } from './env';
import type { Api } from './commands';

export type { Api } from './commands';

export async function createApi(): Promise<Api> {
  if (isTauri()) {
    return (await import('./tauri')).createTauriApi();
  }
  return (await import('./http')).createHttpApi();
}

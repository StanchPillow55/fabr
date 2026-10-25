import { invoke } from '@tauri-apps/api/core';
import { buildApi, type Api } from './commands';
import { ApiError } from './types';

export function createTauriApi(): Api {
  return buildApi((name, _get, req) => invoke('call', { name, req: req ?? {} }).catch((e) => { throw ApiError.from(e); }));
}

export type BackendState = 'connected' | 'stale' | 'absent';

export function backendState(heartbeatAt: string | null, now: Date | string, intervalS = 10): BackendState {
  if (heartbeatAt === null) return 'absent';
  const at = Date.parse(heartbeatAt);
  if (Number.isNaN(at)) return 'absent';
  const age = (new Date(now).getTime() - at) / 1000;
  return age <= 3 * intervalS ? 'connected' : 'stale';
}

export const BACKEND_LABEL: Record<BackendState, string> = {
  connected: 'Agent backend connected',
  stale: 'Agent backend not responding',
  absent: 'Agent backend not connected',
};

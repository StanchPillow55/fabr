import { isTauri } from './env';

export const hasNativePicker = isTauri;

export async function pickFolder(): Promise<string | null> {
  if (!hasNativePicker()) return null;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const r = await open({ directory: true, multiple: false });
  return typeof r === 'string' ? r : null;
}

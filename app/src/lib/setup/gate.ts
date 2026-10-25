import type { FabricSettingsRead } from '../api/types';

export function shouldShowSetup(read: FabricSettingsRead | null): boolean {
  return !read || !read.exists || !read.settings.setup.completed;
}

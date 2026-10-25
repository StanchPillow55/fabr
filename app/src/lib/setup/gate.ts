import type { FabricSettingsRead } from '../api/types';

export function shouldShowSetup(read: FabricSettingsRead): boolean {
  return !read.exists || !read.settings.setup.completed;
}

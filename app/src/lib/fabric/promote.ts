import type { Item } from '../api/types';
import { folderName } from './folder';
import type { CreateFields } from './source';
import type { FabricStore, Held } from './tasks.svelte';

export function promoteFields(item: Item, folder: string): CreateFields {
  return { title: item.title.trim(), due: item.due ?? null, context: `From the todo list in ${folderName(folder)}.` };
}

export type Promoted =
  | { kind: 'done'; held: Held }
  | { kind: 'kept'; held: Held }
  | { kind: 'failed'; error: string };

export const KEPT_MESSAGE = 'The task was created, but the todo could not be removed.';

export async function promote(
  fabric: Pick<FabricStore, 'createTask' | 'createAndDelegate'>,
  folder: string,
  fields: CreateFields,
  delegate: boolean,
  removeTodo: () => Promise<boolean>,
): Promise<Promoted> {
  let held: Held;
  try {
    held = delegate ? await fabric.createAndDelegate(folder, fields) : await fabric.createTask(folder, fields);
  } catch (e) {
    return { kind: 'failed', error: (e as Error).message };
  }
  let removed = false;
  try { removed = await removeTodo(); } catch { /* reported as kept */ }
  return { kind: removed ? 'done' : 'kept', held };
}

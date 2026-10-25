import type { CreateFields } from './source';
import type { FabricStore } from './tasks.svelte';

export class AddFlow {
  text = $state('');
  sheet = $state(false);
  busy = $state(false);
  error = $state<string | null>(null);

  constructor(private fabric: FabricStore, private folder: () => string | null, private disabled: () => boolean = () => false) {}

  get off(): boolean {
    return this.disabled() || !this.folder();
  }

  ask(): void {
    if (this.off || this.sheet || !this.text.trim()) return;
    this.error = null;
    this.sheet = true;
  }

  press(name: string): boolean {
    if (name === 'Enter') {
      this.ask();
      return true;
    }
    if (name === 'Escape' && this.text) {
      this.text = '';
      return true;
    }
    return false;
  }

  cancel(): void {
    this.sheet = false;
  }

  async finish(f: CreateFields, delegate: boolean): Promise<void> {
    const folder = this.folder();
    if (this.busy || !folder) return;
    this.busy = true;
    this.error = null;
    try {
      const held = delegate ? await this.fabric.createAndDelegate(folder, f) : await this.fabric.createTask(folder, f);
      this.text = '';
      this.sheet = false;
      this.fabric.open(held.path);
    } catch (e) {
      this.error = (e as Error).message;
      await this.fabric.load().catch(() => {});
    } finally {
      this.busy = false;
    }
  }
}

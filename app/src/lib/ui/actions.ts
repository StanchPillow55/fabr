export type CommitOpts = (() => void) | { fn: () => void; enter?: 'commit' | 'blur' };

export function commit(node: HTMLElement, opts: CommitOpts) {
  let cfg = typeof opts === 'function' ? { fn: opts } : opts;
  const keydown = (e: KeyboardEvent) => {
    if (e.key !== 'Enter') return;
    e.preventDefault();
    e.stopPropagation();
    if (cfg.enter === 'blur') node.blur();
    else cfg.fn();
  };
  const blur = () => cfg.fn();
  node.addEventListener('keydown', keydown);
  node.addEventListener('blur', blur);
  return {
    update(next: CommitOpts) {
      cfg = typeof next === 'function' ? { fn: next } : next;
    },
    destroy() {
      node.removeEventListener('keydown', keydown);
      node.removeEventListener('blur', blur);
    },
  };
}

import { RefObject, useEffect, useRef } from 'react';

const fieldSelector = 'input:not([type="checkbox"]),select,textarea,[data-terminal-accept]';

export function advanceTerminalFocus(element: HTMLElement): void {
  const root = element.closest<HTMLElement>('[data-terminal-entry]');
  if (!root) return;
  const explicit = root.querySelectorAll<HTMLElement>('[data-terminal-order]');
  const fields = [...(explicit.length ? explicit : root.querySelectorAll<HTMLElement>(fieldSelector))]
    .filter((field) => !field.hasAttribute('disabled') && !field.hidden && !field.closest('[role="dialog"]'))
    .sort((a, b) => Number(a.dataset.terminalOrder || '500') - Number(b.dataset.terminalOrder || '500'));
  const current = fields.indexOf(element);
  if (current >= 0) fields[current + 1]?.focus();
}

interface TerminalOptions {
  container: RefObject<HTMLElement>;
  onAccept: () => void;
  onBack: () => void;
  onInsert?: () => void;
  onSave?: () => void;
  onDate?: () => void;
  onVoucherType?: (type: string) => void;
}

/** Capture field Enter before a browser can submit; custom lists own their Enter. */
export function useTerminalKeyboard({ container, onAccept, onBack, onInsert, onSave, onDate, onVoucherType }: TerminalOptions): void {
  const callbacks = useRef({ onAccept, onBack, onInsert, onSave, onDate, onVoucherType });
  callbacks.current = { onAccept, onBack, onInsert, onSave, onDate, onVoucherType };
  useEffect(() => {
    const handle = (event: KeyboardEvent) => {
      const root = container.current;
      if (!root || event.defaultPrevented) return;
      const target = event.target instanceof HTMLElement ? event.target : document.activeElement as HTMLElement | null;
      if (target && target !== document.body && !root.contains(target)) return;
      if (root.querySelector('[role="dialog"],.ot-modal-overlay,.ot-modal-backdrop')) return;
      if (event.key === 'Escape' && target?.getAttribute('role') === 'combobox' && target.getAttribute('aria-expanded') === 'true') return;
      const typeKeys: Record<string, string> = { F4: 'vt_contra', F5: 'vt_payment', F6: 'vt_receipt', F7: 'vt_jnl', F8: 'vt_sales', F9: 'vt_pur' };
      if (typeKeys[event.key] && !event.ctrlKey && !event.altKey && callbacks.current.onVoucherType) {
        event.preventDefault(); event.stopImmediatePropagation(); callbacks.current.onVoucherType(typeKeys[event.key]);
      } else if (event.key === 'Escape') {
        event.preventDefault(); event.stopImmediatePropagation(); callbacks.current.onBack();
      } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's' && callbacks.current.onSave) {
        event.preventDefault(); event.stopImmediatePropagation(); callbacks.current.onSave();
      } else if (event.key === 'F2' && callbacks.current.onDate) {
        event.preventDefault(); event.stopImmediatePropagation(); callbacks.current.onDate();
      } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') {
        event.preventDefault(); event.stopImmediatePropagation(); callbacks.current.onAccept();
      } else if (event.key === 'Insert' && callbacks.current.onInsert) {
        event.preventDefault(); event.stopImmediatePropagation(); callbacks.current.onInsert();
      } else if (event.key === 'Enter' && !event.shiftKey && !event.ctrlKey && !event.altKey && target?.matches('input,select,textarea') && target.getAttribute('role') !== 'combobox') {
        event.preventDefault(); event.stopImmediatePropagation(); advanceTerminalFocus(target);
      }
    };
    window.addEventListener('keydown', handle, true);
    return () => window.removeEventListener('keydown', handle, true);
  }, [container]);
}

import { useEffect, useRef } from 'react';

interface Props { onYes: () => void; onNo: () => void; busy?: boolean }

export function AcceptVoucherDialog({ onYes, onNo, busy = false }: Props) {
  const yes = useRef<HTMLButtonElement>(null);
  useEffect(() => { yes.current?.focus(); }, []);
  useEffect(() => {
    const handle = (event: KeyboardEvent) => {
      if (event.key === 'Escape' || event.key.toLowerCase() === 'n') {
        event.preventDefault(); event.stopImmediatePropagation(); if (!busy) onNo();
      } else if (event.key.toLowerCase() === 'y' || ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a')) {
        event.preventDefault(); event.stopImmediatePropagation(); if (!busy) onYes();
      }
    };
    window.addEventListener('keydown', handle, true);
    return () => window.removeEventListener('keydown', handle, true);
  }, [busy, onNo, onYes]);
  return <div className="ot-modal-overlay"><section className="ot-modal" role="dialog" aria-modal="true" aria-label="Accept voucher" style={{ width: 380, maxWidth: '90vw', padding: 24, background: '#fff', border: '1px solid #d6e2dc' }}>
    <h3>Accept voucher? [Y/N]</h3><p>Save and post this voucher to the books.</p>
    <button ref={yes} disabled={busy} onClick={onYes}>Yes [Y]</button>{' '}<button disabled={busy} onClick={onNo}>No [N / Esc]</button>
  </section></div>;
}

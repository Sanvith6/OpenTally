import { useCallback, useEffect, useRef, useState } from 'react';

interface PdfExportActionProps {
  filename: string;
  loadPdf: () => Promise<number[]>;
  disabled?: boolean;
}

export function downloadStatementPdf(bytes: number[], filename: string): void {
  if (!bytes.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)
    || String.fromCharCode(...bytes.slice(0, 5)) !== '%PDF-'
    || !String.fromCharCode(...bytes.slice(-1024)).includes('%%EOF')) {
    throw new Error('PDF export returned an incomplete or invalid document. No file was downloaded.');
  }
  const blob = new Blob([new Uint8Array(bytes)], { type: 'application/pdf' });
  const url = URL.createObjectURL(blob);
  try {
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = filename.replace(/[\\/:*?"<>|]/g, '-');
    anchor.click();
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Export the selected company/date scope; Ctrl+P never opens browser printing. */
export function PdfExportAction({ filename, loadPdf, disabled = false }: PdfExportActionProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const inFlight = useRef(false);
  const exportPdf = useCallback(async () => {
    if (disabled || inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setError('');
    try {
      downloadStatementPdf(await loadPdf(), filename);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }, [disabled, filename, loadPdf]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'p') {
        event.preventDefault();
        if (!event.repeat && !document.querySelector('[role="dialog"], .ot-modal-overlay, .ot-modal-backdrop')) {
          void exportPdf();
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [exportPdf]);

  return <div className="ot-pdf-export">
    <button type="button" className="ot-report-action" data-testid="export-statement-pdf"
      disabled={disabled || busy} onClick={() => void exportPdf()}
      title="Exports the full company statement for the selected dates. Screen-only search and group filters are not applied.">
      {busy ? 'Exporting PDF…' : 'Export PDF (Ctrl+P)'}
    </button>
    {error && <div role="alert" className="ot-report-error" style={{ padding: '6px 0' }}>{error}</div>}
  </div>;
}

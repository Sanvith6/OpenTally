import React, { useState, useEffect } from 'react';
import { ChequeTemplateDto } from '../../types';
import { listChequeTemplates, generateChequePdfBytes, exportChequePdf } from '../../services/api';
import './ChequePrintModal.css';

interface ChequePrintModalProps {
  isOpen: boolean;
  voucherId: string;
  voucherNumber?: string;
  companyId: string;
  onClose: () => void;
  onNotification?: (msg: string) => void;
}

export const ChequePrintModal: React.FC<ChequePrintModalProps> = ({
  isOpen,
  voucherId,
  voucherNumber,
  companyId,
  onClose,
  onNotification,
}) => {
  const [templates, setTemplates] = useState<ChequeTemplateDto[]>([]);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>('');
  const [isAccountPayee, setIsAccountPayee] = useState<boolean>(true);
  const [loading, setLoading] = useState<boolean>(false);
  const [outputPath, setOutputPath] = useState<string>('cheque_printout.pdf');

  useEffect(() => {
    if (isOpen) {
      listChequeTemplates(companyId)
        .then((tmpls) => {
          setTemplates(tmpls);
          if (tmpls.length > 0) {
            setSelectedTemplateId(tmpls[0].id);
            setIsAccountPayee(tmpls[0].is_account_payee_default);
          }
        })
        .catch((err) => {
          if (onNotification) onNotification(`Failed to load cheque templates: ${err}`);
        });
    }
  }, [isOpen, companyId, onNotification]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (isOpen && e.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const handlePrintPreview = async () => {
    setLoading(true);
    try {
      const bytes = await generateChequePdfBytes(
        voucherId,
        selectedTemplateId || undefined,
        isAccountPayee,
        companyId
      );
      const blob = new Blob([new Uint8Array(bytes)], { type: 'application/pdf' });
      const url = URL.createObjectURL(blob);
      window.open(url, '_blank');
      if (onNotification) onNotification('CTS-2010 Cheque PDF generated successfully!');
    } catch (err: unknown) {
      if (onNotification) onNotification(err instanceof Error ? err.message : 'Cheque printing failed');
    } finally {
      setLoading(false);
    }
  };

  const handleExport = async () => {
    setLoading(true);
    try {
      const savedPath = await exportChequePdf(
        voucherId,
        outputPath,
        selectedTemplateId || undefined,
        isAccountPayee,
        companyId
      );
      if (onNotification) onNotification(`Cheque exported to: ${savedPath}`);
      onClose();
    } catch (err: unknown) {
      if (onNotification) onNotification(err instanceof Error ? err.message : 'Cheque export failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="ot-cheque-modal-overlay" data-testid="cheque-print-modal">
      <div className="ot-cheque-modal">
        <header className="ot-cheque-modal-header">
          <h3>Print CTS-2010 Bank Cheque</h3>
          <span className="ot-cheque-vch-badge">{voucherNumber || voucherId}</span>
        </header>

        <div className="ot-cheque-modal-body">
          <div className="ot-cheque-field">
            <label>Cheque Template (Bank Dimensions):</label>
            <select
              value={selectedTemplateId}
              onChange={(e) => {
                setSelectedTemplateId(e.target.value);
                const selected = templates.find((t) => t.id === e.target.value);
                if (selected) setIsAccountPayee(selected.is_account_payee_default);
              }}
            >
              {templates.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.bank_name} ({t.width_mm} × {t.height_mm} mm)
                </option>
              ))}
            </select>
          </div>

          <div className="ot-cheque-checkbox-field">
            <label>
              <input
                type="checkbox"
                checked={isAccountPayee}
                onChange={(e) => setIsAccountPayee(e.target.checked)}
              />
              Cross with &quot;A/C PAYEE ONLY&quot; (Parallel diagonal top-left lines)
            </label>
          </div>

          <div className="ot-cheque-field">
            <label>Export File Path (Optional):</label>
            <input
              type="text"
              value={outputPath}
              onChange={(e) => setOutputPath(e.target.value)}
              placeholder="e.g. C:/Cheques/cheque_001.pdf"
            />
          </div>
        </div>

        <footer className="ot-cheque-modal-footer">
          <button type="button" className="ot-btn ot-btn-secondary" onClick={onClose}>
            Esc: Cancel
          </button>
          <button
            type="button"
            className="ot-btn ot-btn-secondary"
            disabled={loading}
            onClick={handleExport}
          >
            Export PDF
          </button>
          <button
            type="button"
            className="ot-btn ot-btn-primary"
            disabled={loading}
            onClick={handlePrintPreview}
          >
            {loading ? 'Generating...' : 'Print / View PDF'}
          </button>
        </footer>
      </div>
    </div>
  );
};

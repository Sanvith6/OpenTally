import { Ledger, VoucherEntryType, VoucherLineInput } from '../../types';

interface VoucherLineGridProps {
  lines: VoucherLineInput[];
  ledgers: Ledger[];
  onAddLine: () => void;
  onUpdateLine: (index: number, updated: Partial<VoucherLineInput>) => void;
  onDeleteLine: (index: number) => void;
  onMoveLineUp: (index: number) => void;
  onMoveLineDown: (index: number) => void;
}

export function VoucherLineGrid({
  lines,
  ledgers,
  onAddLine,
  onUpdateLine,
  onDeleteLine,
  onMoveLineUp,
  onMoveLineDown,
}: VoucherLineGridProps) {
  const handleKeyDown = (
    e: React.KeyboardEvent,
    index: number,
    field: 'entry_type' | 'ledger_id' | 'amount' | 'narration'
  ) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (field === 'entry_type') {
        const nextInput = document.getElementById(`line-ledger-${index}`);
        nextInput?.focus();
      } else if (field === 'ledger_id') {
        const nextInput = document.getElementById(`line-amount-${index}`);
        nextInput?.focus();
      } else if (field === 'amount') {
        const nextInput = document.getElementById(`line-narration-${index}`);
        nextInput?.focus();
      } else if (field === 'narration') {
        if (index === lines.length - 1) {
          onAddLine();
          setTimeout(() => {
            const nextType = document.getElementById(`line-type-${index + 1}`);
            nextType?.focus();
          }, 50);
        } else {
          const nextType = document.getElementById(`line-type-${index + 1}`);
          nextType?.focus();
        }
      }
    } else if (e.key === 'Insert') {
      e.preventDefault();
      onAddLine();
    } else if (e.key === 'Delete' && e.ctrlKey) {
      e.preventDefault();
      onDeleteLine(index);
    }
  };

  return (
    <div className="ot-voucher-grid-container">
      <table className="ot-table ot-voucher-grid-table">
        <thead>
          <tr>
            <th style={{ width: '40px', textAlign: 'center' }}>#</th>
            <th style={{ width: '80px', textAlign: 'center' }}>Dr / Cr</th>
            <th style={{ minWidth: '240px' }}>Particulars (Ledger Account)</th>
            <th style={{ width: '160px', textAlign: 'right' }}>Debit (₹)</th>
            <th style={{ width: '160px', textAlign: 'right' }}>Credit (₹)</th>
            <th>Narration</th>
            <th style={{ width: '100px', textAlign: 'center' }}>Actions</th>
          </tr>
        </thead>
        <tbody>
          {lines.length === 0 ? (
            <tr>
              <td colSpan={7} style={{ textAlign: 'center', padding: '32px', color: '#777' }}>
                No line items added yet. Click &quot;Add Line Item&quot; or press [Insert] to add a line.
              </td>
            </tr>
          ) : (
            lines.map((line, idx) => {
              const isDr = line.entry_type === 'Dr';
              return (
                <tr key={line.id || `line-${idx}`} className="ot-voucher-line-row">
                  <td style={{ textAlign: 'center', fontWeight: 'bold', color: '#555' }}>
                    {idx + 1}
                  </td>

                  {/* Dr / Cr Selector */}
                  <td style={{ textAlign: 'center' }}>
                    <select
                      id={`line-type-${idx}`}
                      className="ot-input ot-entry-type-select"
                      value={line.entry_type}
                      onChange={(e) =>
                        onUpdateLine(idx, { entry_type: e.target.value as VoucherEntryType })
                      }
                      onKeyDown={(e) => handleKeyDown(e, idx, 'entry_type')}
                    >
                      <option value="Dr">Dr</option>
                      <option value="Cr">Cr</option>
                    </select>
                  </td>

                  {/* Ledger Account Combobox */}
                  <td>
                    <select
                      id={`line-ledger-${idx}`}
                      className="ot-input ot-ledger-select"
                      value={line.ledger_id}
                      onChange={(e) => onUpdateLine(idx, { ledger_id: e.target.value })}
                      onKeyDown={(e) => handleKeyDown(e, idx, 'ledger_id')}
                    >
                      <option value="">-- Select Ledger Account --</option>
                      {ledgers.map((l) => (
                        <option key={l.id} value={l.id}>
                          {l.name}
                        </option>
                      ))}
                    </select>
                  </td>

                  {/* Debit Amount */}
                  <td style={{ textAlign: 'right' }}>
                    {isDr ? (
                      <input
                        id={`line-amount-${idx}`}
                        type="text"
                        className="ot-input ot-amount-input"
                        placeholder="0.00"
                        value={line.amount}
                        onChange={(e) => onUpdateLine(idx, { amount: e.target.value })}
                        onKeyDown={(e) => handleKeyDown(e, idx, 'amount')}
                        style={{ textAlign: 'right', fontWeight: '600' }}
                      />
                    ) : (
                      <span className="ot-dash">-</span>
                    )}
                  </td>

                  {/* Credit Amount */}
                  <td style={{ textAlign: 'right' }}>
                    {!isDr ? (
                      <input
                        id={`line-amount-${idx}`}
                        type="text"
                        className="ot-input ot-amount-input"
                        placeholder="0.00"
                        value={line.amount}
                        onChange={(e) => onUpdateLine(idx, { amount: e.target.value })}
                        onKeyDown={(e) => handleKeyDown(e, idx, 'amount')}
                        style={{ textAlign: 'right', fontWeight: '600' }}
                      />
                    ) : (
                      <span className="ot-dash">-</span>
                    )}
                  </td>

                  {/* Line Narration */}
                  <td>
                    <input
                      id={`line-narration-${idx}`}
                      type="text"
                      className="ot-input ot-line-narration-input"
                      placeholder="Line remark..."
                      value={line.narration || ''}
                      onChange={(e) => onUpdateLine(idx, { narration: e.target.value })}
                      onKeyDown={(e) => handleKeyDown(e, idx, 'narration')}
                    />
                  </td>

                  {/* Actions: Reorder and Delete */}
                  <td style={{ textAlign: 'center', whiteSpace: 'nowrap' }}>
                    <button
                      type="button"
                      className="ot-icon-btn"
                      disabled={idx === 0}
                      onClick={() => onMoveLineUp(idx)}
                      title="Move Up"
                    >
                      ▲
                    </button>
                    <button
                      type="button"
                      className="ot-icon-btn"
                      disabled={idx === lines.length - 1}
                      onClick={() => onMoveLineDown(idx)}
                      title="Move Down"
                    >
                      ▼
                    </button>
                    <button
                      type="button"
                      className="ot-icon-btn ot-delete-line-btn"
                      onClick={() => onDeleteLine(idx)}
                      title="Delete Line (Ctrl+Delete)"
                    >
                      ✕
                    </button>
                  </td>
                </tr>
              );
            })
          )}
        </tbody>
      </table>

      <div className="ot-grid-footer-bar" style={{ marginTop: '12px' }}>
        <button
          type="button"
          className="ot-btn ot-btn-secondary"
          onClick={onAddLine}
          title="Add a new debit or credit leg [Insert]"
        >
          + Add Line Item (Insert)
        </button>
      </div>
    </div>
  );
}

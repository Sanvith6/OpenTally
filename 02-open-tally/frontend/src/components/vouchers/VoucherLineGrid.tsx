import { Ledger, VoucherEntryType, VoucherLineInput } from '../../types';
import { SearchableAccountSelect } from './SearchableAccountSelect';
import { minorAmount } from './terminalAmounts';

interface VoucherLineGridProps {
  lines: VoucherLineInput[];
  ledgers: Ledger[];
  onAddLine: () => void;
  onUpdateLine: (index: number, updated: Partial<VoucherLineInput>) => void;
  onDeleteLine: (index: number) => void;
  onMoveLineUp: (index: number) => void;
  onMoveLineDown: (index: number) => void;
  onOpenBillAllocations?: (index: number) => void;
}

export function VoucherLineGrid({
  lines,
  ledgers,
  onAddLine,
  onUpdateLine,
  onDeleteLine,
  onMoveLineUp,
  onMoveLineDown,
  onOpenBillAllocations,
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
                      data-terminal-order={100 + idx * 10}
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
                    <SearchableAccountSelect
                      id={`line-ledger-${idx}`}
                      label={`Ledger account ${idx + 1}`}
                      order={101 + idx * 10}
                      value={line.ledger_id}
                      onChange={(id) => onUpdateLine(idx, { ledger_id: id })}
                      options={ledgers}
                    />
                    {onOpenBillAllocations && line.ledger_id && minorAmount(line.amount) > 0n && (
                      <div style={{ marginTop: '4px' }}>
                        <button
                          type="button"
                          className="ot-badge-btn"
                          onClick={() => onOpenBillAllocations(idx)}
                          title="Configure Bill-wise details (Agst Ref, New Ref)"
                          style={{
                            fontSize: '0.72rem',
                            padding: '1px 6px',
                            background: line.bill_allocations && line.bill_allocations.length > 0 ? '#065f46' : '#1e293b',
                            color: line.bill_allocations && line.bill_allocations.length > 0 ? '#a7f3d0' : '#38bdf8',
                            border: '1px solid #334155',
                            borderRadius: '3px',
                            cursor: 'pointer',
                          }}
                        >
                          Bill-wise {line.bill_allocations && line.bill_allocations.length > 0 ? `(${line.bill_allocations.length} allocated)` : '+'}
                        </button>
                      </div>
                    )}
                    {line.ledger_id && ledgers.find((l) => l.id === line.ledger_id)?.name.toLowerCase().includes('bank') && (
                      <div style={{ marginTop: '4px' }}>
                        <button
                          type="button"
                          className="ot-badge-btn"
                          onClick={() => {
                            const cur = line.bank_allocation || { transaction_type: 'Cheque', instrument_number: '', instrument_date: '' };
                            onUpdateLine(idx, { bank_allocation: line.bank_allocation ? undefined : cur });
                          }}
                          title="Configure Bank instrument details (Cheque, NEFT, RTGS, etc.)"
                          style={{
                            fontSize: '0.72rem',
                            padding: '1px 6px',
                            background: line.bank_allocation ? '#075985' : '#1e293b',
                            color: line.bank_allocation ? '#bae6fd' : '#38bdf8',
                            border: '1px solid #334155',
                            borderRadius: '3px',
                            cursor: 'pointer',
                          }}
                        >
                          Bank Details {line.bank_allocation ? '✓' : '+'}
                        </button>
                      </div>
                    )}
                    {line.bank_allocation && (
                      <div
                        style={{
                          marginTop: '6px',
                          padding: '6px',
                          background: '#0f172a',
                          border: '1px solid #334155',
                          borderRadius: '4px',
                          display: 'flex',
                          gap: '6px',
                          flexWrap: 'wrap',
                          fontSize: '0.75rem',
                        }}
                      >
                        <select
                          className="ot-input"
                          style={{ fontSize: '0.75rem', padding: '2px 4px' }}
                          value={line.bank_allocation.transaction_type || 'Cheque'}
                          onChange={(e) =>
                            onUpdateLine(idx, {
                              bank_allocation: { ...line.bank_allocation, transaction_type: e.target.value },
                            })
                          }
                        >
                          {['Cheque', 'NEFT', 'RTGS', 'IMPS', 'UPI', 'DD', 'Card', 'ECS', 'Other'].map((t) => (
                            <option key={t} value={t}>
                              {t}
                            </option>
                          ))}
                        </select>
                        <input
                          type="text"
                          placeholder="Inst. No."
                          className="ot-input"
                          style={{ fontSize: '0.75rem', padding: '2px 4px', width: '90px' }}
                          value={line.bank_allocation.instrument_number || ''}
                          onChange={(e) =>
                            onUpdateLine(idx, {
                              bank_allocation: { ...line.bank_allocation, instrument_number: e.target.value },
                            })
                          }
                        />
                        <input
                          type="date"
                          placeholder="Inst. Date"
                          className="ot-input"
                          style={{ fontSize: '0.75rem', padding: '2px 4px' }}
                          value={line.bank_allocation.instrument_date || ''}
                          onChange={(e) =>
                            onUpdateLine(idx, {
                              bank_allocation: { ...line.bank_allocation, instrument_date: e.target.value },
                            })
                          }
                        />
                      </div>
                    )}
                  </td>

                  {/* Debit Amount */}
                  <td style={{ textAlign: 'right' }}>
                    {isDr ? (
                      <input
                        id={`line-amount-${idx}`}
                        data-terminal-order={102 + idx * 10}
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
                        data-terminal-order={102 + idx * 10}
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
                      data-terminal-order={103 + idx * 10}
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

import React, { useState, useEffect } from 'react';
import { BillAllocationInput, PendingBill } from '../../types';
import { getPartyPendingBills } from '../../services/api';
import './BillWiseAllocationModal.css';

interface BillWiseAllocationModalProps {
  isOpen: boolean;
  partyLedgerId: string;
  partyLedgerName: string;
  lineAmount: string;
  lineEntryType: 'Dr' | 'Cr';
  initialAllocations: BillAllocationInput[];
  onSaveAllocations: (allocations: BillAllocationInput[]) => void;
  onClose: () => void;
}

export const BillWiseAllocationModal: React.FC<BillWiseAllocationModalProps> = ({
  isOpen,
  partyLedgerId,
  partyLedgerName,
  lineAmount,
  lineEntryType,
  initialAllocations,
  onSaveAllocations,
  onClose,
}) => {
  const [pendingBills, setPendingBills] = useState<PendingBill[]>([]);
  const [allocations, setAllocations] = useState<BillAllocationInput[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const targetAmount = parseFloat(lineAmount) || 0;

  useEffect(() => {
    if (isOpen && partyLedgerId) {
      setLoading(true);
      setError(null);
      const today = new Date().toISOString().substring(0, 10);
      getPartyPendingBills(partyLedgerId, today)
        .then((bills) => {
          setPendingBills(bills);
          if (initialAllocations.length > 0) {
            setAllocations(initialAllocations);
          } else if (bills.length > 0) {
            // Suggest first pending bill as Agst Ref
            const first = bills[0];
            const allocAmt = Math.min(targetAmount, parseFloat(first.pending_amount) || targetAmount);
            setAllocations([
              {
                allocation_type: 'AgstRef',
                bill_name: first.bill_name,
                amount: allocAmt.toFixed(2),
                dr_cr: lineEntryType,
                due_date: first.due_date,
              },
            ]);
          } else {
            // Default to New Ref
            setAllocations([
              {
                allocation_type: 'NewRef',
                bill_name: `BILL-${Date.now().toString().slice(-5)}`,
                amount: targetAmount > 0 ? targetAmount.toFixed(2) : '',
                dr_cr: lineEntryType,
                due_date: null,
              },
            ]);
          }
        })
        .catch((err) => {
          setError(err instanceof Error ? err.message : String(err));
        })
        .finally(() => {
          setLoading(false);
        });
    }
  }, [isOpen, partyLedgerId, targetAmount, lineEntryType, initialAllocations]);

  // Intercept Escape key to dismiss this dialog without popping parent navigation stack
  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('keydown', handleEsc, true);
    return () => window.removeEventListener('keydown', handleEsc, true);
  }, [isOpen, onClose]);

  // Exact monetary arithmetic using integer minor units (paise)
  const toPaise = (val: string | number) =>
    Math.round((typeof val === 'number' ? val : parseFloat(val) || 0) * 100);
  const targetPaise = toPaise(lineAmount);
  const allocatedPaise = allocations.reduce((sum, a) => sum + toPaise(a.amount), 0);
  const differencePaise = Math.abs(targetPaise - allocatedPaise);
  const difference = (differencePaise / 100).toFixed(2);
  const allocatedSum = (allocatedPaise / 100).toFixed(2);
  const isMatch = targetPaise > 0 && targetPaise === allocatedPaise;

  const handleAddRow = () => {
    const remainingPaise = Math.max(0, targetPaise - allocatedPaise);
    setAllocations((prev) => [
      ...prev,
      {
        allocation_type: 'OnAccount',
        bill_name: `REF-${prev.length + 1}`,
        amount: remainingPaise > 0 ? (remainingPaise / 100).toFixed(2) : '0.00',
        dr_cr: lineEntryType,
        due_date: null,
      },
    ]);
  };

  const handleUpdateRow = (index: number, updated: Partial<BillAllocationInput>) => {
    setAllocations((prev) => {
      const next = [...prev];
      next[index] = { ...next[index], ...updated };
      return next;
    });
  };

  const handleDeleteRow = (index: number) => {
    setAllocations((prev) => prev.filter((_, idx) => idx !== index));
  };

  const handleSelectPendingBill = (index: number, billName: string) => {
    const selectedBill = pendingBills.find((b) => b.bill_name === billName);
    if (!selectedBill) return;

    const remainingPaise = Math.max(
      0,
      targetPaise - (allocatedPaise - toPaise(allocations[index]?.amount || '0'))
    );
    const allocAmtPaise = Math.min(
      remainingPaise,
      toPaise(selectedBill.pending_amount) || remainingPaise
    );

    handleUpdateRow(index, {
      bill_name: selectedBill.bill_name,
      amount:
        allocAmtPaise > 0
          ? (allocAmtPaise / 100).toFixed(2)
          : selectedBill.pending_amount,
      due_date: selectedBill.due_date,
    });
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!isMatch) {
      alert(
        `Total allocated (₹${allocatedSum}) must exactly match the voucher line amount (₹${lineAmount}). Difference: ₹${difference}`
      );
      return;
    }
    onSaveAllocations(allocations);
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div className="ot-bill-modal-backdrop" onClick={onClose} data-testid="billwise-allocation-modal">
      <div className="ot-bill-modal-container" onClick={(e) => e.stopPropagation()}>
        <div className="ot-bill-modal-header">
          <div>
            <h3>Bill-wise Details for: {partyLedgerName}</h3>
            <span className="ot-bill-modal-sub">
              Line Amount: ₹{targetAmount.toFixed(2)} ({lineEntryType})
            </span>
          </div>
          <button type="button" className="ot-bill-modal-close" onClick={onClose}>
            &times;
          </button>
        </div>

        <form onSubmit={handleSubmit} className="ot-bill-modal-body">
          {error && <div className="ot-alert ot-alert-danger">{error}</div>}

          {/* Pending Bills Reference Box */}
          {pendingBills.length > 0 && (
            <div className="ot-pending-bills-box">
              <div className="ot-pending-bills-header">
                <span>Available Pending Bills ({pendingBills.length})</span>
              </div>
              <div className="ot-pending-bills-chips">
                {pendingBills.map((b) => (
                  <span
                    key={b.bill_name}
                    className="ot-bill-chip"
                    onClick={() => {
                      // Check if already in allocations
                      const existingIdx = allocations.findIndex((a) => a.bill_name === b.bill_name);
                      if (existingIdx >= 0) {
                        handleSelectPendingBill(existingIdx, b.bill_name);
                      } else {
                        const remainingPaise = Math.max(0, targetPaise - allocatedPaise);
                        const amtPaise = Math.min(
                          remainingPaise,
                          toPaise(b.pending_amount) || remainingPaise
                        );
                        setAllocations((prev) => [
                          ...prev,
                          {
                            allocation_type: 'AgstRef',
                            bill_name: b.bill_name,
                            amount:
                              amtPaise > 0
                                ? (amtPaise / 100).toFixed(2)
                                : b.pending_amount,
                            dr_cr: lineEntryType,
                            due_date: b.due_date,
                          },
                        ]);
                      }
                    }}
                    title={`Click to allocate: ${b.bill_name} (Pending: ₹${b.pending_amount})`}
                  >
                    <strong>{b.bill_name}</strong>: ₹{b.pending_amount} ({b.bill_date})
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Allocations Grid */}
          <table className="ot-bill-grid">
            <thead>
              <tr>
                <th style={{ width: '130px' }}>Type of Ref</th>
                <th>Name / Bill Ref</th>
                <th style={{ width: '110px' }}>Due Date</th>
                <th style={{ width: '140px', textAlign: 'right' }}>Amount (₹)</th>
                <th style={{ width: '40px' }}></th>
              </tr>
            </thead>
            <tbody>
              {allocations.map((alloc, idx) => (
                <tr key={idx}>
                  <td>
                    <select
                      value={alloc.allocation_type}
                      onChange={(e) => {
                        const val = e.target.value as 'AgstRef' | 'NewRef' | 'Advance' | 'OnAccount';
                        handleUpdateRow(idx, {
                          allocation_type: val,
                          bill_name:
                            val === 'AgstRef' && pendingBills.length > 0
                              ? pendingBills[0].bill_name
                              : val === 'NewRef'
                              ? `REF-${Date.now().toString().slice(-4)}`
                              : alloc.bill_name,
                        });
                      }}
                    >
                      <option value="AgstRef">Agst Ref</option>
                      <option value="NewRef">New Ref</option>
                      <option value="Advance">Advance</option>
                      <option value="OnAccount">On Account</option>
                    </select>
                  </td>
                  <td>
                    {alloc.allocation_type === 'AgstRef' && pendingBills.length > 0 ? (
                      <select
                        value={alloc.bill_name}
                        onChange={(e) => handleSelectPendingBill(idx, e.target.value)}
                      >
                        {pendingBills.map((b) => (
                          <option key={b.bill_name} value={b.bill_name}>
                            {b.bill_name} (Pending: ₹{b.pending_amount})
                          </option>
                        ))}
                      </select>
                    ) : (
                      <input
                        type="text"
                        value={alloc.bill_name}
                        onChange={(e) => handleUpdateRow(idx, { bill_name: e.target.value })}
                        placeholder="Bill Reference Name / Number"
                        required
                      />
                    )}
                  </td>
                  <td>
                    <input
                      type="date"
                      value={alloc.due_date || ''}
                      onChange={(e) => handleUpdateRow(idx, { due_date: e.target.value || null })}
                    />
                  </td>
                  <td style={{ textAlign: 'right' }}>
                    <input
                      type="text"
                      className="ot-amount-input"
                      value={alloc.amount}
                      onChange={(e) => handleUpdateRow(idx, { amount: e.target.value })}
                      required
                    />
                  </td>
                  <td>
                    <button
                      type="button"
                      className="ot-del-btn"
                      onClick={() => handleDeleteRow(idx)}
                      title="Remove row"
                    >
                      &times;
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr>
                <td colSpan={2}>
                  <button
                    type="button"
                    className="ot-btn ot-btn-secondary"
                    onClick={handleAddRow}
                  >
                    + Add Allocation
                  </button>
                </td>
                <td style={{ textAlign: 'right', fontWeight: 600 }}>Total:</td>
                <td style={{ textAlign: 'right', fontWeight: 700, color: isMatch ? '#10b981' : '#ef4444' }}>
                  ₹ {allocatedSum}
                </td>
                <td></td>
              </tr>
            </tfoot>
          </table>

          {/* Allocation Equilibrium Warning / Success */}
          <div className="ot-balance-bar">
            {isMatch ? (
              <span className="ot-bal-success">&#10003; Exact match with line amount (₹{lineAmount})</span>
            ) : (
              <span className="ot-bal-error">
                &#9888; Difference: ₹{difference} (Allocated: ₹{allocatedSum} / Required: ₹{lineAmount})
              </span>
            )}
          </div>

          <div className="ot-bill-modal-actions">
            <button type="button" className="ot-btn" onClick={onClose}>
              Cancel
            </button>
            <button
              type="submit"
              className="ot-btn ot-btn-primary"
              disabled={!isMatch || loading}
            >
              Accept Bill Allocations (Ctrl+A)
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

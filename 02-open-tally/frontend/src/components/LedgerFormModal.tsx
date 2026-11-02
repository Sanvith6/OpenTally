import React, { useState, useEffect } from 'react';
import {
  AccountGroup,
  BalanceType,
  CreateLedgerInput,
  Ledger,
  UpdateLedgerInput,
} from '../types';
import {
  createLedger,
  updateLedger,
  setOpeningBalance,
} from '../services/api';

interface LedgerFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSaved: () => void;
  groupOptions: AccountGroup[];
  defaultGroupId?: string;
  editingLedger?: Ledger | null;
  initialBalance?: { balance_type: BalanceType; amount: string } | null;
}

export const LedgerFormModal: React.FC<LedgerFormModalProps> = ({
  isOpen,
  onClose,
  onSaved,
  groupOptions,
  defaultGroupId,
  editingLedger,
  initialBalance,
}) => {
  const [name, setName] = useState('');
  const [groupId, setGroupId] = useState('');
  const [description, setDescription] = useState('');
  const [balanceAmount, setBalanceAmount] = useState('');
  const [balanceType, setBalanceType] = useState<BalanceType>('Dr');
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (isOpen) {
      if (editingLedger) {
        setName(editingLedger.name);
        setGroupId(editingLedger.group_id);
        setDescription(editingLedger.description || '');
        if (initialBalance) {
          setBalanceAmount(initialBalance.amount);
          setBalanceType(initialBalance.balance_type);
        } else {
          setBalanceAmount('');
          setBalanceType('Dr');
        }
      } else {
        setName('');
        setGroupId(defaultGroupId || (groupOptions.length > 0 ? groupOptions[0].id : ''));
        setDescription('');
        setBalanceAmount('');
        setBalanceType('Dr');
      }
      setError(null);
    }
  }, [isOpen, editingLedger, defaultGroupId, groupOptions, initialBalance]);

  if (!isOpen) return null;

  const isReserved = editingLedger?.is_reserved ?? false;

  const handleSubmit = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    const trimmedName = name.trim();
    if (!trimmedName) {
      setError('Ledger name cannot be empty');
      return;
    }
    if (!groupId) {
      setError('Please select an account group');
      return;
    }

    try {
      setIsSubmitting(true);
      setError(null);

      let ledgerId = editingLedger?.id;

      if (editingLedger) {
        const updateReq: UpdateLedgerInput = {
          name: trimmedName,
          group_id: isReserved ? undefined : groupId,
          description: description.trim() || undefined,
        };
        await updateLedger(editingLedger.id, updateReq);
      } else {
        const createReq: CreateLedgerInput = {
          name: trimmedName,
          group_id: groupId,
          description: description.trim() || undefined,
        };
        const created = await createLedger(createReq);
        ledgerId = created.id;
      }

      // Handle opening balance if specified or modified
      if (ledgerId) {
        const amt = balanceAmount.trim();
        await setOpeningBalance({
          ledger_id: ledgerId,
          balance_type: balanceType,
          amount: amt === '' ? '0.00' : amt,
        });
      }

      onSaved();
      onClose();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    } else if (e.key === 'Enter' && e.ctrlKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        backgroundColor: 'rgba(0, 0, 0, 0.75)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
      onKeyDown={handleKeyDown}
    >
      <div
        style={{
          backgroundColor: '#1f2937',
          border: '2px solid #059669',
          borderRadius: '8px',
          width: '500px',
          maxWidth: '90vw',
          padding: '24px',
          color: '#ffffff',
          boxShadow: '0 20px 25px -5px rgba(0, 0, 0, 0.5)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '16px', borderBottom: '1px solid #374151', paddingBottom: '8px' }}>
          <h3 style={{ margin: 0, color: '#34d399', fontSize: '1.1rem' }}>
            {editingLedger ? 'Alter Ledger Master' : 'Create Ledger Master'}
          </h3>
          {isReserved && (
            <span style={{ fontSize: '0.75rem', backgroundColor: '#991b1b', color: '#ffffff', padding: '2px 8px', borderRadius: '4px', fontWeight: 'bold' }}>
              RESERVED SYSTEM LEDGER
            </span>
          )}
        </div>

        {error && (
          <div
            style={{
              padding: '8px 12px',
              backgroundColor: '#7f1d1d',
              border: '1px solid #b91c1c',
              borderRadius: '4px',
              color: '#fecaca',
              fontSize: '0.875rem',
              marginBottom: '16px',
            }}
          >
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit}>
          {/* Ledger Name */}
          <div style={{ marginBottom: '14px' }}>
            <label style={{ display: 'block', fontSize: '0.85rem', color: '#9ca3af', marginBottom: '4px' }}>
              Ledger Name * {isReserved && '(Protected)'}
            </label>
            <input
              type="text"
              autoFocus={!isReserved}
              disabled={isReserved}
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. State Bank Operating A/c"
              style={{
                width: '100%',
                padding: '8px 12px',
                backgroundColor: isReserved ? '#374151' : '#111827',
                border: '1px solid #4b5563',
                borderRadius: '4px',
                color: isReserved ? '#9ca3af' : '#ffffff',
                fontSize: '0.9rem',
                boxSizing: 'border-box',
                cursor: isReserved ? 'not-allowed' : 'text',
              }}
            />
          </div>

          {/* Group Dropdown */}
          <div style={{ marginBottom: '14px' }}>
            <label style={{ display: 'block', fontSize: '0.85rem', color: '#9ca3af', marginBottom: '4px' }}>
              Under Group * {isReserved && '(Protected)'}
            </label>
            <select
              value={groupId}
              disabled={isReserved}
              onChange={(e) => setGroupId(e.target.value)}
              style={{
                width: '100%',
                padding: '8px 12px',
                backgroundColor: isReserved ? '#374151' : '#111827',
                border: '1px solid #4b5563',
                borderRadius: '4px',
                color: isReserved ? '#9ca3af' : '#ffffff',
                fontSize: '0.9rem',
                boxSizing: 'border-box',
                cursor: isReserved ? 'not-allowed' : 'pointer',
              }}
            >
              {groupOptions.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.name} ({g.nature})
                </option>
              ))}
            </select>
          </div>

          {/* Description */}
          <div style={{ marginBottom: '14px' }}>
            <label style={{ display: 'block', fontSize: '0.85rem', color: '#9ca3af', marginBottom: '4px' }}>
              Description / Notes (Optional)
            </label>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="e.g. Operating branch current account"
              style={{
                width: '100%',
                padding: '8px 12px',
                backgroundColor: '#111827',
                border: '1px solid #4b5563',
                borderRadius: '4px',
                color: '#ffffff',
                fontSize: '0.9rem',
                boxSizing: 'border-box',
              }}
            />
          </div>

          {/* Opening Balance Field */}
          <div style={{ marginBottom: '20px' }}>
            <label style={{ display: 'block', fontSize: '0.85rem', color: '#9ca3af', marginBottom: '4px' }}>
              Opening Balance (Enter 0 or leave blank to clear)
            </label>
            <div style={{ display: 'flex', gap: '8px' }}>
              <input
                type="text"
                value={balanceAmount}
                onChange={(e) => setBalanceAmount(e.target.value)}
                placeholder="0.00"
                style={{
                  flex: 1,
                  padding: '8px 12px',
                  backgroundColor: '#111827',
                  border: '1px solid #4b5563',
                  borderRadius: '4px',
                  color: '#ffffff',
                  fontSize: '0.9rem',
                  fontFamily: 'monospace',
                }}
              />
              <button
                type="button"
                onClick={() => setBalanceType(balanceType === 'Dr' ? 'Cr' : 'Dr')}
                style={{
                  width: '60px',
                  backgroundColor: balanceType === 'Dr' ? '#1d4ed8' : '#b91c1c',
                  border: 'none',
                  borderRadius: '4px',
                  color: '#ffffff',
                  fontWeight: 'bold',
                  fontSize: '0.9rem',
                  cursor: 'pointer',
                }}
                title="Toggle Dr / Cr (Click or press D/C)"
              >
                {balanceType}
              </button>
            </div>
          </div>

          {/* Buttons */}
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px', borderTop: '1px solid #374151', paddingTop: '16px' }}>
            <button
              type="button"
              onClick={onClose}
              style={{
                padding: '8px 16px',
                backgroundColor: '#374151',
                border: 'none',
                borderRadius: '4px',
                color: '#ffffff',
                cursor: 'pointer',
              }}
            >
              Cancel (Esc)
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              style={{
                padding: '8px 20px',
                backgroundColor: '#059669',
                border: 'none',
                borderRadius: '4px',
                color: '#ffffff',
                fontWeight: 'bold',
                cursor: isSubmitting ? 'not-allowed' : 'pointer',
              }}
            >
              {isSubmitting ? 'Saving...' : 'Save Ledger (Ctrl+Enter)'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};


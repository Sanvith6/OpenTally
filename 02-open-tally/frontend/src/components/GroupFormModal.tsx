import React, { useState, useEffect } from 'react';
import { AccountGroup, CreateAccountGroupInput, UpdateAccountGroupInput } from '../types';
import { createAccountGroup, updateAccountGroup } from '../services/api';

interface GroupFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSaved: () => void;
  parentOptions: AccountGroup[];
  defaultParentId?: string;
  editingGroup?: AccountGroup | null;
}

export const GroupFormModal: React.FC<GroupFormModalProps> = ({
  isOpen,
  onClose,
  onSaved,
  parentOptions,
  defaultParentId,
  editingGroup,
}) => {
  const [name, setName] = useState('');
  const [parentId, setParentId] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (isOpen) {
      if (editingGroup) {
        setName(editingGroup.name);
        setParentId(editingGroup.parent_id || '');
      } else {
        setName('');
        setParentId(defaultParentId || (parentOptions.length > 0 ? parentOptions[0].id : ''));
      }
      setError(null);
    }
  }, [isOpen, editingGroup, defaultParentId, parentOptions]);

  if (!isOpen) return null;

  const handleSubmit = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) {
      setError('Group name cannot be empty');
      return;
    }
    if (!parentId) {
      setError('Please select a parent group');
      return;
    }

    try {
      setIsSubmitting(true);
      setError(null);
      if (editingGroup) {
        const updateReq: UpdateAccountGroupInput = {
          name: trimmed,
          parent_id: parentId,
        };
        await updateAccountGroup(editingGroup.id, updateReq);
      } else {
        const createReq: CreateAccountGroupInput = {
          name: trimmed,
          parent_id: parentId,
        };
        await createAccountGroup(createReq);
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
          border: '2px solid #0284c7',
          borderRadius: '8px',
          width: '460px',
          maxWidth: '90vw',
          padding: '24px',
          color: '#ffffff',
          boxShadow: '0 20px 25px -5px rgba(0, 0, 0, 0.5)',
        }}
      >
        <h3
          style={{
            margin: '0 0 16px 0',
            color: '#38bdf8',
            fontSize: '1.1rem',
            borderBottom: '1px solid #374151',
            paddingBottom: '8px',
          }}
        >
          {editingGroup ? 'Alter Account Group' : 'Create Account Group'}
        </h3>

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
          <div style={{ marginBottom: '16px' }}>
            <label
              style={{
                display: 'block',
                fontSize: '0.85rem',
                color: '#9ca3af',
                marginBottom: '4px',
              }}
            >
              Group Name *
            </label>
            <input
              type="text"
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Northern Debtors"
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

          <div style={{ marginBottom: '24px' }}>
            <label
              style={{
                display: 'block',
                fontSize: '0.85rem',
                color: '#9ca3af',
                marginBottom: '4px',
              }}
            >
              Parent Group (Under) *
            </label>
            <select
              value={parentId}
              onChange={(e) => setParentId(e.target.value)}
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
            >
              {parentOptions.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.name} ({g.nature})
                </option>
              ))}
            </select>
          </div>

          <div
            style={{
              display: 'flex',
              justifyContent: 'flex-end',
              gap: '12px',
              borderTop: '1px solid #374151',
              paddingTop: '16px',
            }}
          >
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
                backgroundColor: '#0284c7',
                border: 'none',
                borderRadius: '4px',
                color: '#ffffff',
                fontWeight: 'bold',
                cursor: isSubmitting ? 'not-allowed' : 'pointer',
              }}
            >
              {isSubmitting ? 'Saving...' : 'Save Group (Ctrl+Enter)'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};


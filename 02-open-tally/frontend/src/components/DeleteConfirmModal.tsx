import React from 'react';
import { ChartOfAccountsTreeNode } from '../types';

interface DeleteConfirmModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  node: ChartOfAccountsTreeNode | null;
  isDeleting: boolean;
  error?: string | null;
}

export const DeleteConfirmModal: React.FC<DeleteConfirmModalProps> = ({
  isOpen,
  onClose,
  onConfirm,
  node,
  isDeleting,
  error,
}) => {
  if (!isOpen || !node) return null;

  const isGroup = node.node_type === 'group';

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      onConfirm();
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
        backgroundColor: 'rgba(0, 0, 0, 0.8)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
      onKeyDown={handleKeyDown}
      tabIndex={0}
      autoFocus
    >
      <div
        style={{
          backgroundColor: '#1f2937',
          border: '2px solid #ef4444',
          borderRadius: '8px',
          width: '450px',
          maxWidth: '90vw',
          padding: '24px',
          color: '#ffffff',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.7)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '16px' }}>
          <span style={{ fontSize: '1.75rem', color: '#ef4444' }}>⚠️</span>
          <h3 style={{ margin: 0, color: '#f87171', fontSize: '1.2rem' }}>
            Confirm Deletion
          </h3>
        </div>

        {error && (
          <div
            style={{
              padding: '10px 14px',
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

        <p style={{ margin: '0 0 12px 0', fontSize: '0.95rem', color: '#e5e7eb' }}>
          Are you sure you want to permanently delete this {isGroup ? 'account group' : 'ledger'}?
        </p>

        <div
          style={{
            padding: '12px',
            backgroundColor: '#111827',
            borderRadius: '6px',
            border: '1px solid #374151',
            marginBottom: '20px',
          }}
        >
          <div style={{ fontWeight: 'bold', fontSize: '1rem', color: '#ffffff' }}>
            {node.name}
          </div>
          <div style={{ fontSize: '0.8rem', color: '#9ca3af', marginTop: '4px' }}>
            Type: {isGroup ? (node.is_primary ? 'Primary Group' : 'Sub-Group') : (node.is_reserved ? 'Reserved System Ledger' : 'Standard Ledger')}
            {node.nature ? ` | Nature: ${node.nature}` : ''}
          </div>
        </div>

        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
          <button
            type="button"
            onClick={onClose}
            disabled={isDeleting}
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
            type="button"
            onClick={onConfirm}
            disabled={isDeleting}
            style={{
              padding: '8px 20px',
              backgroundColor: '#dc2626',
              border: 'none',
              borderRadius: '4px',
              color: '#ffffff',
              fontWeight: 'bold',
              cursor: isDeleting ? 'not-allowed' : 'pointer',
            }}
          >
            {isDeleting ? 'Deleting...' : 'Delete Permanently (Enter)'}
          </button>
        </div>
      </div>
    </div>
  );
};


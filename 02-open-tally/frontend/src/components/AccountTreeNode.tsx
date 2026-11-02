import React from 'react';
import { ChartOfAccountsTreeNode } from '../types';

interface AccountTreeNodeProps {
  node: ChartOfAccountsTreeNode;
  depth: number;
  isSelected: boolean;
  isExpanded: boolean;
  onToggleExpand: (nodeId: string) => void;
  onSelectNode: (nodeId: string) => void;
  onEditLedger: (node: ChartOfAccountsTreeNode) => void;
}

export const AccountTreeNode: React.FC<AccountTreeNodeProps> = ({
  node,
  depth,
  isSelected,
  isExpanded,
  onToggleExpand,
  onSelectNode,
  onEditLedger,
}) => {
  const isGroup = node.node_type === 'group';
  const hasChildren = node.children && node.children.length > 0;

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onSelectNode(node.id);
  };

  const handleDoubleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isGroup) {
      onToggleExpand(node.id);
    } else {
      onEditLedger(node);
    }
  };

  const getNatureBadgeColor = (nature: string) => {
    switch (nature) {
      case 'Assets':
        return '#3b82f6'; // Blue
      case 'Liabilities':
        return '#ef4444'; // Red
      case 'Income':
        return '#10b981'; // Green
      case 'Expenses':
        return '#f59e0b'; // Amber
      default:
        return '#6b7280';
    }
  };

  return (
    <div
      className={`tree-row ${isSelected ? 'selected' : ''}`}
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      data-testid={`tree-node-${node.id}`}
      style={{
        display: 'flex',
        alignItems: 'center',
        padding: '5px 10px',
        paddingLeft: `${depth * 22 + 10}px`,
        cursor: 'pointer',
        backgroundColor: isSelected ? 'var(--tally-selection-bg, #0284c7)' : 'transparent',
        color: isSelected ? '#ffffff' : 'inherit',
        borderBottom: '1px solid rgba(255, 255, 255, 0.05)',
        fontSize: '0.875rem',
        userSelect: 'none',
      }}
    >
      {/* Expand/Collapse Chevron */}
      <span
        style={{
          width: '18px',
          display: 'inline-block',
          cursor: isGroup ? 'pointer' : 'default',
          color: isSelected ? '#ffffff' : '#9ca3af',
          fontWeight: 'bold',
        }}
        onClick={(e) => {
          if (isGroup) {
            e.stopPropagation();
            onToggleExpand(node.id);
          }
        }}
      >
        {isGroup ? (hasChildren ? (isExpanded ? '▼' : '▶') : '•') : ' '}
      </span>

      {/* Node Type Indicator */}
      <span
        style={{
          marginRight: '8px',
          fontSize: '0.75rem',
          padding: '1px 5px',
          borderRadius: '3px',
          backgroundColor: isGroup
            ? node.is_primary
              ? '#374151'
              : '#1f2937'
            : node.is_reserved
            ? '#b91c1c'
            : '#065f46',
          color: '#ffffff',
          fontWeight: 'bold',
        }}
      >
        {isGroup
          ? node.is_primary
            ? 'PRIMARY'
            : 'SUB-GROUP'
          : node.is_reserved
          ? 'RESERVED'
          : 'LEDGER'}
      </span>

      {/* Node Name */}
      <span
        style={{
          flex: 1,
          fontWeight: isGroup ? 'bold' : 'normal',
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
        }}
      >
        {node.name}
      </span>

      {/* Nature Badge */}
      {node.nature && (
        <span
          style={{
            fontSize: '0.75rem',
            padding: '1px 6px',
            borderRadius: '10px',
            backgroundColor: 'transparent',
            border: `1px solid ${getNatureBadgeColor(node.nature)}`,
            color: isSelected ? '#ffffff' : getNatureBadgeColor(node.nature),
            marginRight: '20px',
          }}
        >
          {node.nature}
        </span>
      )}

      {/* Debit Column */}
      <span
        style={{
          width: '120px',
          textAlign: 'right',
          fontFamily: 'monospace',
          color: parseFloat(node.debit_total) > 0 ? (isSelected ? '#ffffff' : '#60a5fa') : '#6b7280',
        }}
      >
        {parseFloat(node.debit_total) > 0 ? `${node.debit_total} Dr` : '—'}
      </span>

      {/* Credit Column */}
      <span
        style={{
          width: '120px',
          textAlign: 'right',
          fontFamily: 'monospace',
          color: parseFloat(node.credit_total) > 0 ? (isSelected ? '#ffffff' : '#f87171') : '#6b7280',
        }}
      >
        {parseFloat(node.credit_total) > 0 ? `${node.credit_total} Cr` : '—'}
      </span>
    </div>
  );
};


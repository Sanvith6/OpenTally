import React, { useMemo } from 'react';
import { ChartOfAccountsTreeNode } from '../types';
import { AccountTreeNode } from './AccountTreeNode';

export interface FlatRow {
  node: ChartOfAccountsTreeNode;
  depth: number;
}

interface AccountTreeViewProps {
  nodes: ChartOfAccountsTreeNode[];
  expandedNodeIds: Set<string>;
  selectedNodeId: string | null;
  searchFilter: string;
  onSearchChange: (query: string) => void;
  onToggleExpand: (nodeId: string) => void;
  onSelectNode: (nodeId: string) => void;
  onEditLedger: (node: ChartOfAccountsTreeNode) => void;
  onOpenCreateGroup: () => void;
  onOpenCreateLedger: () => void;
  onOpenDeleteConfirm: () => void;
}

export const AccountTreeView: React.FC<AccountTreeViewProps> = ({
  nodes,
  expandedNodeIds,
  selectedNodeId,
  searchFilter,
  onSearchChange,
  onToggleExpand,
  onSelectNode,
  onEditLedger,
  onOpenCreateGroup,
  onOpenCreateLedger,
  onOpenDeleteConfirm,
}) => {
  // Flatten tree according to expanded state and search filter
  const flatRows = useMemo(() => {
    const rows: FlatRow[] = [];
    const filter = searchFilter.trim().toLowerCase();

    function traverse(node: ChartOfAccountsTreeNode, depth: number) {
      const matches = !filter || node.name.toLowerCase().includes(filter);

      // If search filter is active, show matching node regardless of expansion
      if (filter) {
        if (matches) {
          rows.push({ node, depth });
        }
        for (const child of node.children) {
          traverse(child, depth + 1);
        }
      } else {
        rows.push({ node, depth });
        if (expandedNodeIds.has(node.id) && node.children.length > 0) {
          for (const child of node.children) {
            traverse(child, depth + 1);
          }
        }
      }
    }

    for (const root of nodes) {
      traverse(root, 0);
    }

    return rows;
  }, [nodes, expandedNodeIds, searchFilter]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Search and Action Toolbar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '8px 12px',
          backgroundColor: '#1f2937',
          borderBottom: '1px solid #374151',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flex: 1 }}>
          <span style={{ color: '#9ca3af', fontSize: '0.875rem' }}>🔍</span>
          <input
            id="coa-search-input"
            type="text"
            placeholder="Search Groups & Ledgers (Ctrl+F)..."
            value={searchFilter}
            onChange={(e) => onSearchChange(e.target.value)}
            style={{
              padding: '4px 8px',
              backgroundColor: '#111827',
              border: '1px solid #4b5563',
              borderRadius: '4px',
              color: '#ffffff',
              fontSize: '0.875rem',
              width: '280px',
            }}
          />
          {searchFilter && (
            <button
              onClick={() => onSearchChange('')}
              style={{
                backgroundColor: 'transparent',
                border: 'none',
                color: '#9ca3af',
                cursor: 'pointer',
              }}
            >
              ✕
            </button>
          )}
        </div>

        <div style={{ display: 'flex', gap: '8px' }}>
          <button
            onClick={onOpenCreateGroup}
            style={{
              padding: '4px 10px',
              backgroundColor: '#0284c7',
              border: 'none',
              borderRadius: '4px',
              color: '#ffffff',
              fontSize: '0.8rem',
              cursor: 'pointer',
              fontWeight: 'bold',
            }}
            title="Create sub-group under highlighted node (Alt+C / Ctrl+G)"
          >
            + Group (Alt+C)
          </button>
          <button
            onClick={onOpenCreateLedger}
            style={{
              padding: '4px 10px',
              backgroundColor: '#059669',
              border: 'none',
              borderRadius: '4px',
              color: '#ffffff',
              fontSize: '0.8rem',
              cursor: 'pointer',
              fontWeight: 'bold',
            }}
            title="Create ledger under highlighted group (Ctrl+L)"
          >
            + Ledger (Ctrl+L)
          </button>
          <button
            onClick={onOpenDeleteConfirm}
            style={{
              padding: '4px 10px',
              backgroundColor: '#dc2626',
              border: 'none',
              borderRadius: '4px',
              color: '#ffffff',
              fontSize: '0.8rem',
              cursor: 'pointer',
              fontWeight: 'bold',
            }}
            title="Delete highlighted group or ledger (Del)"
          >
            Delete (Del)
          </button>
        </div>
      </div>

      {/* Column Headers */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          padding: '6px 12px',
          backgroundColor: '#111827',
          borderBottom: '2px solid #374151',
          fontWeight: 'bold',
          fontSize: '0.8rem',
          color: '#9ca3af',
          textTransform: 'uppercase',
          letterSpacing: '0.05em',
        }}
      >
        <span style={{ width: '18px' }} />
        <span style={{ width: '80px', marginRight: '8px' }}>Type</span>
        <span style={{ flex: 1 }}>Account Group / Ledger Name</span>
        <span style={{ width: '90px', marginRight: '20px' }}>Nature</span>
        <span style={{ width: '120px', textAlign: 'right' }}>Debit Total</span>
        <span style={{ width: '120px', textAlign: 'right' }}>Credit Total</span>
      </div>

      {/* Tree Rows Linear Scroll Container */}
      <div
        id="coa-tree-scroll-container"
        tabIndex={0}
        style={{
          flex: 1,
          overflowY: 'auto',
          backgroundColor: '#0f172a',
          outline: 'none',
        }}
      >
        {flatRows.length === 0 ? (
          <div
            style={{
              padding: '40px',
              textAlign: 'center',
              color: '#9ca3af',
              fontSize: '0.9rem',
            }}
          >
            {searchFilter ? `No groups or ledgers match "${searchFilter}"` : 'No accounts available.'}
          </div>
        ) : (
          flatRows.map(({ node, depth }) => (
            <AccountTreeNode
              key={node.id}
              node={node}
              depth={depth}
              isSelected={selectedNodeId === node.id}
              isExpanded={expandedNodeIds.has(node.id)}
              onToggleExpand={onToggleExpand}
              onSelectNode={onSelectNode}
              onEditLedger={onEditLedger}
            />
          ))
        )}
      </div>
    </div>
  );
};


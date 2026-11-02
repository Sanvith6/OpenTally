import React, { useState, useEffect, useCallback, useMemo } from 'react';
import {
  AccountGroup,
  ChartOfAccountsTree,
  ChartOfAccountsTreeNode,
  Company,
  Ledger,
} from '../types';
import {
  getChartOfAccounts,
  listAccountGroups,
  deleteAccountGroup,
  deleteLedger,
  getLedger,
} from '../services/api';
import { AccountTreeView, FlatRow } from './AccountTreeView';
import { GroupFormModal } from './GroupFormModal';
import { LedgerFormModal } from './LedgerFormModal';
import { DeleteConfirmModal } from './DeleteConfirmModal';
import { EquilibriumStatusBar } from './EquilibriumStatusBar';
import { ChartOfAccountsHeader } from './ChartOfAccountsHeader';
import { useChartOfAccountsKeyboard } from '../hooks/useChartOfAccountsKeyboard';

interface ChartOfAccountsViewProps {
  activeCompany: Company | null;
  onBackToGateway: () => void;
}

export const ChartOfAccountsView: React.FC<ChartOfAccountsViewProps> = ({
  activeCompany,
  onBackToGateway,
}) => {
  const [treeData, setTreeData] = useState<ChartOfAccountsTree | null>(null);
  const [groups, setGroups] = useState<AccountGroup[]>([]);
  const [expandedNodeIds, setExpandedNodeIds] = useState<Set<string>>(new Set());
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const initialSearch =
    typeof window !== 'undefined'
      ? new URLSearchParams(window.location.search).get('search') || ''
      : '';
  const [searchFilter, setSearchFilter] = useState(initialSearch);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Modals state
  const initialModal =
    typeof window !== 'undefined'
      ? new URLSearchParams(window.location.search).get('modal')
      : null;
  const [isGroupModalOpen, setIsGroupModalOpen] = useState(initialModal === 'group');
  const [editingGroup, setEditingGroup] = useState<AccountGroup | null>(null);
  const [isLedgerModalOpen, setIsLedgerModalOpen] = useState(initialModal === 'ledger');
  const [editingLedger, setEditingLedger] = useState<Ledger | null>(null);
  const [editingLedgerOpb, setEditingLedgerOpb] = useState<{
    balance_type: 'Dr' | 'Cr';
    amount: string;
  } | null>(null);
  const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false);
  const [nodeToDelete, setNodeToDelete] = useState<ChartOfAccountsTreeNode | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  const loadData = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);
      const [tree, grpList] = await Promise.all([
        getChartOfAccounts(),
        listAccountGroups(),
      ]);
      setTreeData(tree);
      setGroups(grpList);

      // Default expand all primary groups
      const initialExpanded = new Set<string>();
      for (const node of tree.nodes) {
        initialExpanded.add(node.id);
      }
      setExpandedNodeIds(initialExpanded);

      setSelectedNodeId((prev) => prev || (tree.nodes.length > 0 ? tree.nodes[0].id : null));
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData, activeCompany]);

  const handleToggleExpand = useCallback((nodeId: string) => {
    setExpandedNodeIds((prev) => {
      const next = new Set(prev);
      if (next.has(nodeId)) {
        next.delete(nodeId);
      } else {
        next.add(nodeId);
      }
      return next;
    });
  }, []);

  // Linear flattened visible rows for keyboard navigation
  const flatRows: FlatRow[] = useMemo(() => {
    if (!treeData) return [];
    const rows: FlatRow[] = [];
    const filter = searchFilter.trim().toLowerCase();

    function traverse(node: ChartOfAccountsTreeNode, depth: number) {
      const matches = !filter || node.name.toLowerCase().includes(filter);
      if (filter) {
        if (matches) rows.push({ node, depth });
        for (const child of node.children) traverse(child, depth + 1);
      } else {
        rows.push({ node, depth });
        if (expandedNodeIds.has(node.id) && node.children.length > 0) {
          for (const child of node.children) traverse(child, depth + 1);
        }
      }
    }

    for (const root of treeData.nodes) {
      traverse(root, 0);
    }
    return rows;
  }, [treeData, expandedNodeIds, searchFilter]);

  const selectedNode = useMemo(() => {
    if (!selectedNodeId) return null;
    const found = flatRows.find((r) => r.node.id === selectedNodeId);
    return found ? found.node : null;
  }, [flatRows, selectedNodeId]);

  const handleOpenEditLedger = useCallback(async (node: ChartOfAccountsTreeNode) => {
    try {
      const ledger = await getLedger(node.id);
      setEditingLedger(ledger);
      setEditingLedgerOpb(
        node.opening_balance
          ? {
              balance_type: node.opening_balance.balance_type,
              amount: node.opening_balance.amount,
            }
          : null
      );
      setIsLedgerModalOpen(true);
    } catch (err: unknown) {
      console.error('Failed to load ledger details:', err);
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const handleOpenCreateGroup = useCallback(() => {
    setEditingGroup(null);
    setIsGroupModalOpen(true);
  }, []);

  const handleOpenCreateLedger = useCallback(() => {
    setEditingLedger(null);
    setEditingLedgerOpb(null);
    setIsLedgerModalOpen(true);
  }, []);

  const handleOpenDeleteConfirm = useCallback(() => {
    if (!selectedNode) return;
    setNodeToDelete(selectedNode);
    setDeleteError(null);
    setIsDeleteModalOpen(true);
  }, [selectedNode]);

  const handleConfirmDelete = useCallback(async () => {
    if (!nodeToDelete) return;
    try {
      setIsDeleting(true);
      setDeleteError(null);
      if (nodeToDelete.node_type === 'group') {
        await deleteAccountGroup(nodeToDelete.id);
      } else {
        await deleteLedger(nodeToDelete.id);
      }
      setIsDeleteModalOpen(false);
      setNodeToDelete(null);
      await loadData();
    } catch (err: unknown) {
      setDeleteError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsDeleting(false);
    }
  }, [nodeToDelete, loadData]);

  // Connect Keyboard Navigation Hook
  useChartOfAccountsKeyboard({
    isModalOpen: isGroupModalOpen || isLedgerModalOpen || isDeleteModalOpen,
    flatRows,
    selectedNodeId,
    selectedNode,
    setSelectedNodeId,
    expandedNodeIds,
    handleToggleExpand,
    handleOpenEditLedger,
    handleOpenCreateGroup,
    handleOpenCreateLedger,
    handleOpenDeleteConfirm,
    onBackToGateway,
  });

  const defaultParentGroupId = selectedNode
    ? selectedNode.node_type === 'group'
      ? selectedNode.id
      : selectedNode.parent_id || undefined
    : undefined;

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        backgroundColor: '#0f172a',
        color: '#ffffff',
      }}
    >
      {/* Top Header */}
      <ChartOfAccountsHeader
        activeCompany={activeCompany}
        onBackToGateway={onBackToGateway}
      />

      {/* Main Tree Area */}
      <div style={{ flex: 1, overflow: 'hidden' }}>
        {isLoading ? (
          <div style={{ padding: '40px', textAlign: 'center', color: '#94a3b8' }}>
            Loading Chart of Accounts...
          </div>
        ) : error ? (
          <div style={{ padding: '30px', color: '#f87171', textAlign: 'center' }}>
            Error loading Chart of Accounts: {error}
          </div>
        ) : treeData ? (
          <AccountTreeView
            nodes={treeData.nodes}
            expandedNodeIds={expandedNodeIds}
            selectedNodeId={selectedNodeId}
            searchFilter={searchFilter}
            onSearchChange={setSearchFilter}
            onToggleExpand={handleToggleExpand}
            onSelectNode={setSelectedNodeId}
            onOpenCreateGroup={handleOpenCreateGroup}
            onOpenCreateLedger={handleOpenCreateLedger}
            onEditLedger={handleOpenEditLedger}
            onOpenDeleteConfirm={handleOpenDeleteConfirm}
          />
        ) : null}
      </div>

      {/* Sticky Bottom Status Bar (Opening Balance Totals & Equilibrium) */}
      <EquilibriumStatusBar
        activeCompany={activeCompany}
        equilibrium={treeData ? treeData.equilibrium : null}
      />

      {/* Group Form Modal */}
      <GroupFormModal
        isOpen={isGroupModalOpen}
        onClose={() => setIsGroupModalOpen(false)}
        onSaved={loadData}
        parentOptions={groups}
        defaultParentId={defaultParentGroupId}
        editingGroup={editingGroup}
      />

      {/* Ledger Form Modal */}
      <LedgerFormModal
        isOpen={isLedgerModalOpen}
        onClose={() => setIsLedgerModalOpen(false)}
        onSaved={loadData}
        groupOptions={groups}
        defaultGroupId={defaultParentGroupId}
        editingLedger={editingLedger}
        initialBalance={editingLedgerOpb}
      />

      {/* Delete Confirmation Modal */}
      <DeleteConfirmModal
        isOpen={isDeleteModalOpen}
        onClose={() => setIsDeleteModalOpen(false)}
        onConfirm={handleConfirmDelete}
        node={nodeToDelete}
        isDeleting={isDeleting}
        error={deleteError}
      />
    </div>
  );
};

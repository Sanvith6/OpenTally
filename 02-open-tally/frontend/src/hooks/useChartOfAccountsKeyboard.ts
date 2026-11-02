import { useEffect } from 'react';
import { ChartOfAccountsTreeNode } from '../types';
import { FlatRow } from '../components/AccountTreeView';

interface UseChartOfAccountsKeyboardProps {
  isModalOpen: boolean;
  flatRows: FlatRow[];
  selectedNodeId: string | null;
  selectedNode: ChartOfAccountsTreeNode | null;
  setSelectedNodeId: (id: string) => void;
  expandedNodeIds: Set<string>;
  handleToggleExpand: (id: string) => void;
  handleOpenEditLedger: (node: ChartOfAccountsTreeNode) => void;
  handleOpenCreateGroup: () => void;
  handleOpenCreateLedger: () => void;
  handleOpenDeleteConfirm: () => void;
  onBackToGateway: () => void;
}

export function useChartOfAccountsKeyboard({
  isModalOpen,
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
}: UseChartOfAccountsKeyboardProps) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Do not capture keyboard if any modal is open
      if (isModalOpen) return;

      const target = e.target as HTMLElement;
      const isInput =
        target &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'SELECT' ||
          target.tagName === 'TEXTAREA');

      if (e.key === 'f' && e.ctrlKey) {
        e.preventDefault();
        const searchInput = document.getElementById('coa-search-input');
        if (searchInput) searchInput.focus();
        return;
      }

      if (isInput) {
        if (e.key === 'Escape') {
          target.blur();
        }
        return;
      }

      if (e.key === 'Escape') {
        e.preventDefault();
        onBackToGateway();
        return;
      }

      if ((e.key === 'c' && e.altKey) || (e.key === 'g' && e.ctrlKey)) {
        e.preventDefault();
        handleOpenCreateGroup();
        return;
      }

      if (e.key === 'l' && e.ctrlKey) {
        e.preventDefault();
        handleOpenCreateLedger();
        return;
      }

      if (e.key === 'Delete') {
        e.preventDefault();
        handleOpenDeleteConfirm();
        return;
      }

      // Tree Arrow Navigation & Space / Enter
      if (flatRows.length === 0) return;
      const currentIndex = flatRows.findIndex((r) => r.node.id === selectedNodeId);

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        const nextIndex =
          currentIndex === -1 ? 0 : Math.min(flatRows.length - 1, currentIndex + 1);
        setSelectedNodeId(flatRows[nextIndex].node.id);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        const prevIndex =
          currentIndex === -1 ? 0 : Math.max(0, currentIndex - 1);
        setSelectedNodeId(flatRows[prevIndex].node.id);
      } else if (e.key === ' ' && selectedNode && selectedNode.node_type === 'group') {
        e.preventDefault();
        handleToggleExpand(selectedNode.id);
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        if (selectedNode && selectedNode.node_type === 'group') {
          if (!expandedNodeIds.has(selectedNode.id)) {
            handleToggleExpand(selectedNode.id);
          } else if (currentIndex + 1 < flatRows.length) {
            setSelectedNodeId(flatRows[currentIndex + 1].node.id);
          }
        }
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        if (
          selectedNode &&
          selectedNode.node_type === 'group' &&
          expandedNodeIds.has(selectedNode.id)
        ) {
          handleToggleExpand(selectedNode.id);
        } else if (selectedNode && selectedNode.parent_id) {
          setSelectedNodeId(selectedNode.parent_id);
        }
      } else if (e.key === 'Enter') {
        e.preventDefault();
        if (selectedNode) {
          if (selectedNode.node_type === 'group') {
            handleToggleExpand(selectedNode.id);
          } else {
            handleOpenEditLedger(selectedNode);
          }
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    isModalOpen,
    flatRows,
    selectedNodeId,
    selectedNode,
    expandedNodeIds,
    handleToggleExpand,
    handleOpenEditLedger,
    handleOpenCreateGroup,
    handleOpenCreateLedger,
    handleOpenDeleteConfirm,
    onBackToGateway,
    setSelectedNodeId,
  ]);
}


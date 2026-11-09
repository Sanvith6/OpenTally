import { useState, useEffect, useCallback } from 'react';
import { VoucherSummary, VoucherType, VoucherFilter } from '../../types';
import { listVoucherDrafts, deleteVoucherDraft } from '../../services/api';

interface DraftsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSelectDraft: (voucherId: string) => void;
  voucherTypes: VoucherType[];
}

export function DraftsModal({
  isOpen,
  onClose,
  onSelectDraft,
  voucherTypes,
}: DraftsModalProps) {
  const [drafts, setDrafts] = useState<VoucherSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [typeFilter, setTypeFilter] = useState('');
  const [searchDate, setSearchDate] = useState('');

  const loadDrafts = useCallback(async () => {
    if (!isOpen) return;
    setLoading(true);
    setError(null);
    try {
      const filter: VoucherFilter = {
        status: 'draft',
        page,
        page_size: 10,
        voucher_type_id: typeFilter || null,
        date_from: searchDate || null,
        date_to: searchDate || null,
      };
      const res = await listVoucherDrafts(filter);
      setDrafts(res.items);
      setTotalPages(res.total_pages);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [isOpen, page, typeFilter, searchDate]);

  useEffect(() => {
    loadDrafts();
  }, [loadDrafts]);

  const handleDelete = async (e: React.MouseEvent, voucherId: string) => {
    e.stopPropagation();
    if (window.confirm('Are you sure you want to delete this draft voucher?')) {
      try {
        await deleteVoucherDraft(voucherId);
        loadDrafts();
      } catch (err) {
        alert(err instanceof Error ? err.message : String(err));
      }
    }
  };

  if (!isOpen) return null;

  return (
    <div className="ot-modal-backdrop" onClick={onClose} role="dialog" aria-modal="true">
      <div
        className="ot-modal-container ot-drafts-modal"
        onClick={(e) => e.stopPropagation()}
        style={{ maxWidth: '850px', width: '90%' }}
      >
        <div className="ot-modal-header">
          <h3>Saved Voucher Drafts</h3>
          <button className="ot-modal-close" onClick={onClose} aria-label="Close modal">
            &times;
          </button>
        </div>

        <div className="ot-modal-body">
          <div className="ot-drafts-filter-bar" style={{ display: 'flex', gap: '12px', marginBottom: '16px' }}>
            <div style={{ flex: 1 }}>
              <label style={{ fontSize: '12px', display: 'block', marginBottom: '4px' }}>Filter by Type</label>
              <select
                className="ot-input"
                value={typeFilter}
                onChange={(e) => {
                  setTypeFilter(e.target.value);
                  setPage(1);
                }}
              >
                <option value="">All Voucher Types</option>
                {voucherTypes.map((vt) => (
                  <option key={vt.id} value={vt.id}>
                    {vt.name}
                  </option>
                ))}
              </select>
            </div>
            <div style={{ flex: 1 }}>
              <label style={{ fontSize: '12px', display: 'block', marginBottom: '4px' }}>Filter by Date</label>
              <input
                type="date"
                className="ot-input"
                value={searchDate}
                onChange={(e) => {
                  setSearchDate(e.target.value);
                  setPage(1);
                }}
              />
            </div>
          </div>

          {error && <div className="ot-alert ot-alert-danger">{error}</div>}

          {loading ? (
            <div style={{ padding: '24px', textAlign: 'center' }}>Loading drafts...</div>
          ) : drafts.length === 0 ? (
            <div style={{ padding: '32px', textAlign: 'center', color: '#888' }}>
              No saved draft vouchers found.
            </div>
          ) : (
            <table className="ot-table" style={{ width: '100%' }}>
              <thead>
                <tr>
                  <th>Voucher #</th>
                  <th>Type</th>
                  <th>Series</th>
                  <th>Date</th>
                  <th>Lines</th>
                  <th>Last Modified</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {drafts.map((d) => (
                  <tr
                    key={d.id}
                    onClick={() => {
                      onSelectDraft(d.id);
                      onClose();
                    }}
                    style={{ cursor: 'pointer' }}
                    className="ot-draft-row"
                  >
                    <td>
                      <span className="ot-badge-clean" style={{ fontSize: '11px' }}>
                        {d.voucher_number}
                      </span>
                    </td>
                    <td>{d.voucher_type_name}</td>
                    <td>{d.series_name}</td>
                    <td>{d.date}</td>
                    <td>{d.line_count} lines</td>
                    <td style={{ fontSize: '12px', color: '#666' }}>
                      {new Date(d.updated_at).toLocaleDateString()}
                    </td>
                    <td>
                      <button
                        className="ot-btn ot-btn-danger"
                        style={{ padding: '3px 8px', fontSize: '11px' }}
                        onClick={(e) => handleDelete(e, d.id)}
                      >
                        Delete
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          {totalPages > 1 && (
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '16px' }}>
              <button
                className="ot-btn"
                disabled={page <= 1}
                onClick={() => setPage((p) => Math.max(1, p - 1))}
              >
                &larr; Previous
              </button>
              <span>
                Page {page} of {totalPages}
              </span>
              <button
                className="ot-btn"
                disabled={page >= totalPages}
                onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
              >
                Next &rarr;
              </button>
            </div>
          )}
        </div>

        <div className="ot-modal-footer">
          <button className="ot-btn" onClick={onClose}>
            Close (Esc)
          </button>
        </div>
      </div>
    </div>
  );
}


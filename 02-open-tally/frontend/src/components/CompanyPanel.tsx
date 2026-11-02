import { Company } from '../types';

interface CompanyPanelProps {
  activeCompany: Company | null;
  onOpenSelectModal: () => void;
  onOpenCreateModal: () => void;
}

export function CompanyPanel({
  activeCompany,
  onOpenSelectModal,
  onOpenCreateModal,
}: CompanyPanelProps) {
  return (
    <div className="ot-company-panel">
      <div className="ot-panel-header">
        <h3>Current Operating Context</h3>
        <span className="ot-badge-clean">
          {activeCompany ? 'Company Loaded' : 'No Company Active'}
        </span>
      </div>

      <div className="ot-company-details">
        <div className="ot-detail-row">
          <span className="ot-detail-label">Current Company:</span>
          {activeCompany ? (
            <span className="ot-detail-value ot-fw-bold ot-text-primary">
              {activeCompany.name}
            </span>
          ) : (
            <span className="ot-detail-value ot-text-muted">[No Company Loaded]</span>
          )}
        </div>

        {activeCompany?.mailing_name && activeCompany.mailing_name !== activeCompany.name && (
          <div className="ot-detail-row">
            <span className="ot-detail-label">Mailing Name:</span>
            <span className="ot-detail-value">{activeCompany.mailing_name}</span>
          </div>
        )}

        <div className="ot-detail-row">
          <span className="ot-detail-label">Financial Year:</span>
          <span className="ot-detail-value">
            {activeCompany
              ? `${activeCompany.financial_year_begins} (Active FY)`
              : '01-Apr-2024 to 31-Mar-2025'}
          </span>
        </div>

        <div className="ot-detail-row">
          <span className="ot-detail-label">Books Beginning:</span>
          <span className="ot-detail-value">
            {activeCompany ? activeCompany.books_beginning_from : '01-Apr-2024'}
          </span>
        </div>

        <div className="ot-detail-row">
          <span className="ot-detail-label">Base Currency:</span>
          <span className="ot-detail-value">
            {activeCompany
              ? `${activeCompany.currency_formal_name} (${activeCompany.currency_symbol}) • Decimal Precision: ${activeCompany.currency_decimal_places}`
              : 'INR (₹) • Decimal Precision: 2'}
          </span>
        </div>

        <div className="ot-detail-row">
          <span className="ot-detail-label">Jurisdiction:</span>
          <span className="ot-detail-value">
            {activeCompany
              ? `${activeCompany.state}, ${activeCompany.country}`
              : 'India (GST Domain)'}
          </span>
        </div>

        <div className="ot-detail-row">
          <span className="ot-detail-label">Security & Audit:</span>
          <span className="ot-detail-value ot-text-emerald">
            Multi-Tenant Isolation & Immutable Audit Log Active
          </span>
        </div>
      </div>

      <div className="ot-company-actions">
        <button className="ot-btn ot-btn-primary" onClick={onOpenSelectModal}>
          <kbd>F3</kbd> Select Company
        </button>
        <button className="ot-btn ot-btn-secondary" onClick={onOpenCreateModal}>
          <kbd>Alt+F3</kbd> Create Company
        </button>
      </div>
    </div>
  );
}

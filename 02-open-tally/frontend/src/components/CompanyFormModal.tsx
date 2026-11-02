import React, { useState } from 'react';
import { CreateCompanyInput, Company } from '../types';
import { createCompany } from '../services/api';

interface CompanyFormModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCompanyCreated: (company: Company) => void;
}

export function CompanyFormModal({
  isOpen,
  onClose,
  onCompanyCreated,
}: CompanyFormModalProps) {
  const [name, setName] = useState('');
  const [mailingName, setMailingName] = useState('');
  const [country, setCountry] = useState('India');
  const [state, setState] = useState('Karnataka');
  const [fyBegins, setFyBegins] = useState('2026-04-01');
  const [booksBegins, setBooksBegins] = useState('2026-04-01');
  const [currencySymbol, setCurrencySymbol] = useState('₹');
  const [currencyFormalName, setCurrencyFormalName] = useState('INR');
  const [decimalPlaces, setDecimalPlaces] = useState(2);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setError('Company name is required');
      return;
    }

    setSubmitting(true);
    setError(null);

    const input: CreateCompanyInput = {
      name: name.trim(),
      mailing_name: mailingName.trim() || undefined,
      country: country.trim() || 'India',
      state: state.trim() || 'Karnataka',
      financial_year_begins: fyBegins.trim(),
      books_beginning_from: booksBegins.trim() || undefined,
      currency_symbol: currencySymbol.trim() || '₹',
      currency_formal_name: currencyFormalName.trim() || 'INR',
      currency_decimal_places: decimalPlaces,
    };

    try {
      const created = await createCompany(input);
      onCompanyCreated(created);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="ot-modal-backdrop" onClick={onClose}>
      <div className="ot-modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="ot-modal-header">
          <h2>Create Company Master</h2>
          <button className="ot-btn-close" onClick={onClose}>
            &times;
          </button>
        </div>

        {error && <div className="ot-modal-error">{error}</div>}

        <form onSubmit={handleSubmit} className="ot-form">
          <div className="ot-form-group">
            <label>Company Name *</label>
            <input
              type="text"
              required
              autoFocus
              value={name}
              placeholder="e.g. Acme Corp Ltd"
              onChange={(e) => {
                setName(e.target.value);
                if (!mailingName || mailingName === name) {
                  setMailingName(e.target.value);
                }
              }}
            />
          </div>

          <div className="ot-form-group">
            <label>Mailing Name</label>
            <input
              type="text"
              value={mailingName}
              placeholder="Leave empty to use Company Name"
              onChange={(e) => setMailingName(e.target.value)}
            />
          </div>

          <div className="ot-form-row">
            <div className="ot-form-group">
              <label>Country</label>
              <input
                type="text"
                value={country}
                onChange={(e) => setCountry(e.target.value)}
              />
            </div>
            <div className="ot-form-group">
              <label>State</label>
              <input
                type="text"
                value={state}
                onChange={(e) => setState(e.target.value)}
              />
            </div>
          </div>

          <div className="ot-form-row">
            <div className="ot-form-group">
              <label>Financial Year Begins (YYYY-MM-DD) *</label>
              <input
                type="date"
                required
                value={fyBegins}
                onChange={(e) => {
                  setFyBegins(e.target.value);
                  if (booksBegins === fyBegins) {
                    setBooksBegins(e.target.value);
                  }
                }}
              />
            </div>
            <div className="ot-form-group">
              <label>Books Beginning From (YYYY-MM-DD) *</label>
              <input
                type="date"
                required
                value={booksBegins}
                onChange={(e) => setBooksBegins(e.target.value)}
              />
            </div>
          </div>

          <div className="ot-form-row">
            <div className="ot-form-group">
              <label>Currency Symbol</label>
              <input
                type="text"
                value={currencySymbol}
                onChange={(e) => setCurrencySymbol(e.target.value)}
              />
            </div>
            <div className="ot-form-group">
              <label>Formal Name</label>
              <input
                type="text"
                value={currencyFormalName}
                onChange={(e) => setCurrencyFormalName(e.target.value)}
              />
            </div>
            <div className="ot-form-group">
              <label>Decimals (0-4)</label>
              <input
                type="number"
                min={0}
                max={4}
                value={decimalPlaces}
                onChange={(e) => setDecimalPlaces(parseInt(e.target.value, 10) || 0)}
              />
            </div>
          </div>

          <div className="ot-modal-footer">
            <button
              type="button"
              className="ot-btn ot-btn-secondary"
              onClick={onClose}
              disabled={submitting}
            >
              Cancel <kbd>Esc</kbd>
            </button>
            <button
              type="submit"
              className="ot-btn ot-btn-primary"
              disabled={submitting}
            >
              {submitting ? 'Creating Company...' : 'Accept (Create Company)'} <kbd>Enter</kbd>
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}


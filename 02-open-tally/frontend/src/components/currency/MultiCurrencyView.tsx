import React, { useState, useEffect, useCallback } from 'react';
import {
  Currency,
  CreateCurrencyInput,
  ExchangeRate,
  SetExchangeRateInput,
  OpenForeignBill,
  SettleForeignBillInput,
  UnrealizedForexReport,
  ChequeTemplateDto,
  Company,
} from '../../types';
import {
  listCurrencies,
  createCurrency,
  listExchangeRates,
  setExchangeRate,
  listOpenForeignBills,
  settleForeignBillWithForex,
  getUnrealizedForexReport,
  listChequeTemplates,
  seedCompanyCurrencyDefaults,
} from '../../services/api';
import './MultiCurrencyView.css';

interface MultiCurrencyViewProps {
  activeCompany: Company | null;
  onBack: () => void;
}

export const MultiCurrencyView: React.FC<MultiCurrencyViewProps> = ({
  activeCompany,
  onBack,
}) => {
  const [activeTab, setActiveTab] = useState<'currencies' | 'rates' | 'bills' | 'revaluation' | 'cheques'>('currencies');
  const [currencies, setCurrencies] = useState<Currency[]>([]);
  const [rates, setRates] = useState<ExchangeRate[]>([]);
  const [openBills, setOpenBills] = useState<OpenForeignBill[]>([]);
  const [revaluationReport, setRevaluationReport] = useState<UnrealizedForexReport | null>(null);
  const [chequeTemplates, setChequeTemplates] = useState<ChequeTemplateDto[]>([]);
  const [revaluationDate, setRevaluationDate] = useState<string>('2026-05-31');

  // New Currency Form State
  const [showAddCurrency, setShowAddCurrency] = useState(false);
  const [newCurrency, setNewCurrency] = useState<CreateCurrencyInput>({
    code: '',
    symbol: '',
    formal_name: '',
    decimal_places: 2,
  });

  // Set Rate Form State
  const [showSetRate, setShowSetRate] = useState(false);
  const [rateInput, setRateInput] = useState<SetExchangeRateInput>({
    currency_code: 'USD',
    effective_date: new Date().toISOString().slice(0, 10),
    standard_rate: '84.00',
    selling_rate: '84.50',
    buying_rate: '83.50',
  });

  // Settle Bill Modal State
  const [settleModalBill, setSettleModalBill] = useState<OpenForeignBill | null>(null);
  const [settleInput, setSettleInput] = useState<{
    foreign_settled_amount: string;
    settlement_date: string;
    settlement_exchange_rate: string;
    payment_mode: string;
    bank_or_cash_ledger_id: string;
    narration: string;
    cheque_instrument_number: string;
  }>({
    foreign_settled_amount: '',
    settlement_date: new Date().toISOString().slice(0, 10),
    settlement_exchange_rate: '',
    payment_mode: 'Bank',
    bank_or_cash_ledger_id: '',
    narration: '',
    cheque_instrument_number: '',
  });

  const [loading, setLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setErrorMsg(null);
    try {
      const [curs, exRates, bills, chqs] = await Promise.all([
        listCurrencies(activeCompany.id),
        listExchangeRates(undefined, activeCompany.id),
        listOpenForeignBills(undefined, activeCompany.id),
        listChequeTemplates(activeCompany.id),
      ]);
      setCurrencies(curs);
      setRates(exRates);
      setOpenBills(bills);
      setChequeTemplates(chqs);

      // Also load revaluation report
      const rep = await getUnrealizedForexReport(revaluationDate, activeCompany.id);
      setRevaluationReport(rep);
    } catch (err: unknown) {
      setErrorMsg(err instanceof Error ? err.message : 'Failed to load multi-currency data');
    } finally {
      setLoading(false);
    }
  }, [activeCompany, revaluationDate]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Global Escape Key to go back to Gateway
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (settleModalBill) {
          setSettleModalBill(null);
        } else if (showAddCurrency) {
          setShowAddCurrency(false);
        } else if (showSetRate) {
          setShowSetRate(false);
        } else {
          onBack();
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [settleModalBill, showAddCurrency, showSetRate, onBack]);

  const handleSeedDefaults = async () => {
    if (!activeCompany) return;
    setLoading(true);
    setErrorMsg(null);
    try {
      const summary = await seedCompanyCurrencyDefaults(activeCompany.id);
      setSuccessMsg(`Seeded default currencies (${summary.currencies_seeded_count}) & cheque templates (${summary.cheque_templates_seeded_count})!`);
      await loadData();
    } catch (err: unknown) {
      setErrorMsg(err instanceof Error ? err.message : 'Failed to seed currency defaults');
    } finally {
      setLoading(false);
    }
  };

  const handleCreateCurrency = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    setErrorMsg(null);
    try {
      await createCurrency(newCurrency, activeCompany.id);
      setSuccessMsg(`Currency ${newCurrency.code} created successfully!`);
      setShowAddCurrency(false);
      setNewCurrency({ code: '', symbol: '', formal_name: '', decimal_places: 2 });
      await loadData();
    } catch (err: unknown) {
      setErrorMsg(err instanceof Error ? err.message : 'Failed to create currency');
    }
  };

  const handleSetRate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    setErrorMsg(null);
    try {
      await setExchangeRate(rateInput, activeCompany.id);
      setSuccessMsg(`Exchange rate for ${rateInput.currency_code} on ${rateInput.effective_date} saved!`);
      setShowSetRate(false);
      await loadData();
    } catch (err: unknown) {
      setErrorMsg(err instanceof Error ? err.message : 'Failed to set exchange rate');
    }
  };

  const handleOpenSettle = (bill: OpenForeignBill) => {
    setSettleModalBill(bill);
    setSettleInput({
      foreign_settled_amount: bill.outstanding_foreign,
      settlement_date: new Date().toISOString().slice(0, 10),
      settlement_exchange_rate: bill.invoice_rate,
      payment_mode: 'Bank',
      bank_or_cash_ledger_id: '',
      narration: `Settlement for foreign bill ${bill.bill_name}`,
      cheque_instrument_number: '',
    });
  };

  const handleExecuteSettlement = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany || !settleModalBill) return;
    setErrorMsg(null);
    try {
      const input: SettleForeignBillInput = {
        bill_name: settleModalBill.bill_name,
        party_ledger_id: settleModalBill.party_ledger_id,
        foreign_settled_amount: settleInput.foreign_settled_amount,
        settlement_date: settleInput.settlement_date,
        settlement_exchange_rate: settleInput.settlement_exchange_rate || undefined,
        payment_mode: settleInput.payment_mode,
        bank_or_cash_ledger_id: settleInput.bank_or_cash_ledger_id,
        narration: settleInput.narration || undefined,
        cheque_instrument_number: settleInput.cheque_instrument_number || undefined,
      };
      const res = await settleForeignBillWithForex(input, activeCompany.id);
      setSuccessMsg(
        `Bill ${settleModalBill.bill_name} settled! Realized Forex: ${res.is_gain ? '+' : '-'}${res.forex_difference} INR (${res.is_gain ? 'Gain' : 'Loss'})`
      );
      setSettleModalBill(null);
      await loadData();
    } catch (err: unknown) {
      setErrorMsg(err instanceof Error ? err.message : 'Settlement failed');
    }
  };

  return (
    <div className="ot-multicurrency-container" data-testid="multicurrency-view">
      <header className="ot-mc-header">
        <div className="ot-mc-header-left">
          <h2>Multi-Currency & Forex Management</h2>
          <span className="ot-mc-company-badge">{activeCompany?.name || 'No Company Selected'}</span>
        </div>
        <div className="ot-mc-header-actions">
          <button className="ot-btn ot-btn-secondary" onClick={handleSeedDefaults} title="Seed INR, USD, EUR, GBP, AED & CTS-2010 Templates">
            Seed Defaults
          </button>
          <button className="ot-btn ot-btn-secondary" onClick={onBack} title="Return to Gateway (Esc)">
            Esc: Back
          </button>
        </div>
      </header>

      {errorMsg && <div className="ot-mc-alert ot-mc-alert-error">{errorMsg}</div>}
      {successMsg && <div className="ot-mc-alert ot-mc-alert-success">{successMsg}</div>}

      <div className="ot-mc-tabs">
        <button
          className={`ot-mc-tab-btn ${activeTab === 'currencies' ? 'active' : ''}`}
          onClick={() => setActiveTab('currencies')}
        >
          Currencies ({currencies.length})
        </button>
        <button
          className={`ot-mc-tab-btn ${activeTab === 'rates' ? 'active' : ''}`}
          onClick={() => setActiveTab('rates')}
        >
          Exchange Rates ({rates.length})
        </button>
        <button
          className={`ot-mc-tab-btn ${activeTab === 'bills' ? 'active' : ''}`}
          onClick={() => setActiveTab('bills')}
        >
          Foreign Bills ({openBills.length})
        </button>
        <button
          className={`ot-mc-tab-btn ${activeTab === 'revaluation' ? 'active' : ''}`}
          onClick={() => setActiveTab('revaluation')}
        >
          Unrealized Forex Revaluation
        </button>
        <button
          className={`ot-mc-tab-btn ${activeTab === 'cheques' ? 'active' : ''}`}
          onClick={() => setActiveTab('cheques')}
        >
          CTS-2010 Cheque Templates ({chequeTemplates.length})
        </button>
      </div>

      <div className="ot-mc-body">
        {loading && <div className="ot-mc-loading">Loading multi-currency records...</div>}

        {/* Tab 1: Currencies */}
        {activeTab === 'currencies' && !loading && (
          <div className="ot-mc-section">
            <div className="ot-mc-section-toolbar">
              <h3>Configured Currencies</h3>
              <button className="ot-btn ot-btn-primary" onClick={() => setShowAddCurrency(true)}>
                + New Currency
              </button>
            </div>

            <table className="ot-mc-table">
              <thead>
                <tr>
                  <th>ISO Code</th>
                  <th>Symbol</th>
                  <th>Formal Name</th>
                  <th>Decimals</th>
                  <th>Base Currency</th>
                  <th>Created At</th>
                </tr>
              </thead>
              <tbody>
                {currencies.map((c) => (
                  <tr key={c.id}>
                    <td><strong>{c.code}</strong></td>
                    <td className="ot-mc-symbol">{c.symbol}</td>
                    <td>{c.formal_name}</td>
                    <td>{c.decimal_places}</td>
                    <td>
                      {c.is_base ? (
                        <span className="ot-mc-badge ot-mc-badge-base">BASE</span>
                      ) : (
                        <span className="ot-mc-badge">FOREIGN</span>
                      )}
                    </td>
                    <td>{c.created_at?.slice(0, 10)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* Tab 2: Daily Rates */}
        {activeTab === 'rates' && !loading && (
          <div className="ot-mc-section">
            <div className="ot-mc-section-toolbar">
              <h3>Daily Exchange Rates</h3>
              <button className="ot-btn ot-btn-primary" onClick={() => setShowSetRate(true)}>
                + Set Rate
              </button>
            </div>

            <table className="ot-mc-table">
              <thead>
                <tr>
                  <th>Currency</th>
                  <th>Effective Date</th>
                  <th>Standard Rate (INR)</th>
                  <th>Selling Rate</th>
                  <th>Buying Rate</th>
                </tr>
              </thead>
              <tbody>
                {rates.map((r) => (
                  <tr key={r.id}>
                    <td><strong>{r.currency_code}</strong></td>
                    <td>{r.effective_date}</td>
                    <td className="ot-mc-num">₹{r.standard_rate}</td>
                    <td className="ot-mc-num">₹{r.selling_rate}</td>
                    <td className="ot-mc-num">₹{r.buying_rate}</td>
                  </tr>
                ))}
                {rates.length === 0 && (
                  <tr>
                    <td colSpan={5} className="ot-mc-empty">No daily exchange rates recorded yet.</td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}

        {/* Tab 3: Open Foreign Bills */}
        {activeTab === 'bills' && !loading && (
          <div className="ot-mc-section">
            <div className="ot-mc-section-toolbar">
              <h3>Open Foreign Currency Bills</h3>
            </div>

            <table className="ot-mc-table">
              <thead>
                <tr>
                  <th>Bill Name</th>
                  <th>Party</th>
                  <th>Currency</th>
                  <th>Invoice Rate</th>
                  <th>Outstanding Foreign</th>
                  <th>Book Value (INR)</th>
                  <th>Dr/Cr</th>
                  <th>Action</th>
                </tr>
              </thead>
              <tbody>
                {openBills.map((b) => (
                  <tr key={b.bill_name}>
                    <td><strong>{b.bill_name}</strong></td>
                    <td>{b.party_name}</td>
                    <td>{b.currency_code}</td>
                    <td className="ot-mc-num">₹{b.invoice_rate}</td>
                    <td className="ot-mc-num">{b.currency_code} {b.outstanding_foreign}</td>
                    <td className="ot-mc-num">₹{b.outstanding_inr}</td>
                    <td><span className={`ot-mc-badge ${b.dr_cr === 'Dr' ? 'ot-mc-dr' : 'ot-mc-cr'}`}>{b.dr_cr}</span></td>
                    <td>
                      <button className="ot-btn ot-btn-sm ot-btn-primary" onClick={() => handleOpenSettle(b)}>
                        Settle with Forex
                      </button>
                    </td>
                  </tr>
                ))}
                {openBills.length === 0 && (
                  <tr>
                    <td colSpan={8} className="ot-mc-empty">No open foreign currency bills found.</td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}

        {/* Tab 4: Unrealized Forex Revaluation */}
        {activeTab === 'revaluation' && !loading && (
          <div className="ot-mc-section">
            <div className="ot-mc-section-toolbar">
              <h3>Unrealized Forex Revaluation Report</h3>
              <div className="ot-mc-date-picker">
                <label>As of Date:</label>
                <input
                  type="date"
                  value={revaluationDate}
                  onChange={(e) => setRevaluationDate(e.target.value)}
                />
                <button className="ot-btn ot-btn-secondary" onClick={loadData}>Recompute</button>
              </div>
            </div>

            {revaluationReport && (
              <>
                <div className="ot-mc-summary-cards">
                  <div className="ot-mc-card">
                    <span className="ot-mc-card-title">Total Book Value</span>
                    <span className="ot-mc-card-val">₹{revaluationReport.total_book_value_inr}</span>
                  </div>
                  <div className="ot-mc-card">
                    <span className="ot-mc-card-title">Total Revalued Value</span>
                    <span className="ot-mc-card-val">₹{revaluationReport.total_revalued_value_inr}</span>
                  </div>
                  <div className="ot-mc-card ot-mc-card-gain">
                    <span className="ot-mc-card-title">Unrealized Gain</span>
                    <span className="ot-mc-card-val">+₹{revaluationReport.total_unrealized_gain}</span>
                  </div>
                  <div className="ot-mc-card ot-mc-card-loss">
                    <span className="ot-mc-card-title">Unrealized Loss</span>
                    <span className="ot-mc-card-val">-₹{revaluationReport.total_unrealized_loss}</span>
                  </div>
                  <div className="ot-mc-card ot-mc-card-net">
                    <span className="ot-mc-card-title">Net Unrealized Gain/Loss</span>
                    <span className="ot-mc-card-val">₹{revaluationReport.net_unrealized_gain_loss}</span>
                  </div>
                </div>

                <table className="ot-mc-table">
                  <thead>
                    <tr>
                      <th>Bill Name</th>
                      <th>Party</th>
                      <th>Currency</th>
                      <th>Open Foreign</th>
                      <th>Invoice Rate</th>
                      <th>Book Value (INR)</th>
                      <th>Revaluation Rate</th>
                      <th>Revalued Value (INR)</th>
                      <th>Unrealized Gain/Loss</th>
                    </tr>
                  </thead>
                  <tbody>
                    {revaluationReport.bills.map((row) => (
                      <tr key={row.bill_name}>
                        <td><strong>{row.bill_name}</strong></td>
                        <td>{row.party_name}</td>
                        <td>{row.currency_code}</td>
                        <td className="ot-mc-num">{row.currency_code} {row.open_foreign_amount}</td>
                        <td className="ot-mc-num">₹{row.invoice_rate}</td>
                        <td className="ot-mc-num">₹{row.book_value_inr}</td>
                        <td className="ot-mc-num">₹{row.revaluation_rate}</td>
                        <td className="ot-mc-num">₹{row.revalued_value_inr}</td>
                        <td className={`ot-mc-num ${row.is_gain ? 'ot-mc-gain' : 'ot-mc-loss'}`}>
                          {row.is_gain ? '+' : '-'}₹{row.unrealized_gain_or_loss}
                        </td>
                      </tr>
                    ))}
                    {revaluationReport.bills.length === 0 && (
                      <tr>
                        <td colSpan={9} className="ot-mc-empty">Zero open foreign currency exposure on {revaluationDate}.</td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </>
            )}
          </div>
        )}

        {/* Tab 5: Cheque Templates */}
        {activeTab === 'cheques' && !loading && (
          <div className="ot-mc-section">
            <div className="ot-mc-section-toolbar">
              <h3>CTS-2010 Bank Cheque Dimensions & Coordinates</h3>
            </div>

            <table className="ot-mc-table">
              <thead>
                <tr>
                  <th>Bank Name</th>
                  <th>Dimensions (mm)</th>
                  <th>Date (X, Y)</th>
                  <th>Payee (X, Y)</th>
                  <th>Words (X, Y)</th>
                  <th>Amount (X, Y)</th>
                  <th>A/C Payee Default</th>
                </tr>
              </thead>
              <tbody>
                {chequeTemplates.map((t) => (
                  <tr key={t.id}>
                    <td><strong>{t.bank_name}</strong></td>
                    <td>{t.width_mm} × {t.height_mm} mm</td>
                    <td>({t.date_x_mm}, {t.date_y_mm})</td>
                    <td>({t.payee_x_mm}, {t.payee_y_mm})</td>
                    <td>({t.words_x_mm}, {t.words_y_mm})</td>
                    <td>({t.amount_x_mm}, {t.amount_y_mm})</td>
                    <td>{t.is_account_payee_default ? 'Yes' : 'No'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Modal: Add Currency */}
      {showAddCurrency && (
        <div className="ot-mc-modal-overlay">
          <div className="ot-mc-modal">
            <h3>Add New Currency Master</h3>
            <form onSubmit={handleCreateCurrency}>
              <div className="ot-mc-form-row">
                <label>ISO 4217 Code:</label>
                <input
                  type="text"
                  maxLength={3}
                  required
                  placeholder="e.g. JPY, CAD"
                  value={newCurrency.code}
                  onChange={(e) => setNewCurrency({ ...newCurrency, code: e.target.value.toUpperCase() })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Symbol:</label>
                <input
                  type="text"
                  required
                  placeholder="e.g. ¥, C$"
                  value={newCurrency.symbol}
                  onChange={(e) => setNewCurrency({ ...newCurrency, symbol: e.target.value })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Formal Name:</label>
                <input
                  type="text"
                  required
                  placeholder="e.g. Japanese Yen"
                  value={newCurrency.formal_name}
                  onChange={(e) => setNewCurrency({ ...newCurrency, formal_name: e.target.value })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Decimal Places:</label>
                <input
                  type="number"
                  min={0}
                  max={4}
                  value={newCurrency.decimal_places}
                  onChange={(e) => setNewCurrency({ ...newCurrency, decimal_places: parseInt(e.target.value) || 2 })}
                />
              </div>
              <div className="ot-mc-modal-actions">
                <button type="button" className="ot-btn ot-btn-secondary" onClick={() => setShowAddCurrency(false)}>Cancel</button>
                <button type="submit" className="ot-btn ot-btn-primary">Create Currency</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Modal: Set Exchange Rate */}
      {showSetRate && (
        <div className="ot-mc-modal-overlay">
          <div className="ot-mc-modal">
            <h3>Set Daily Exchange Rate</h3>
            <form onSubmit={handleSetRate}>
              <div className="ot-mc-form-row">
                <label>Currency:</label>
                <select
                  value={rateInput.currency_code}
                  onChange={(e) => setRateInput({ ...rateInput, currency_code: e.target.value })}
                >
                  {currencies.filter((c) => !c.is_base).map((c) => (
                    <option key={c.code} value={c.code}>{c.code} - {c.formal_name}</option>
                  ))}
                </select>
              </div>
              <div className="ot-mc-form-row">
                <label>Effective Date:</label>
                <input
                  type="date"
                  required
                  value={rateInput.effective_date}
                  onChange={(e) => setRateInput({ ...rateInput, effective_date: e.target.value })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Standard Rate (1 Foreign = INR):</label>
                <input
                  type="text"
                  required
                  placeholder="e.g. 84.20"
                  value={rateInput.standard_rate}
                  onChange={(e) => setRateInput({ ...rateInput, standard_rate: e.target.value })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Selling Rate (Optional):</label>
                <input
                  type="text"
                  placeholder="e.g. 84.50"
                  value={rateInput.selling_rate || ''}
                  onChange={(e) => setRateInput({ ...rateInput, selling_rate: e.target.value })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Buying Rate (Optional):</label>
                <input
                  type="text"
                  placeholder="e.g. 83.80"
                  value={rateInput.buying_rate || ''}
                  onChange={(e) => setRateInput({ ...rateInput, buying_rate: e.target.value })}
                />
              </div>
              <div className="ot-mc-modal-actions">
                <button type="button" className="ot-btn ot-btn-secondary" onClick={() => setShowSetRate(false)}>Cancel</button>
                <button type="submit" className="ot-btn ot-btn-primary">Save Rate</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Modal: Settle Bill with Forex */}
      {settleModalBill && (
        <div className="ot-mc-modal-overlay">
          <div className="ot-mc-modal">
            <h3>Settle Foreign Bill: {settleModalBill.bill_name}</h3>
            <p className="ot-mc-modal-subtitle">
              Party: {settleModalBill.party_name} | Currency: {settleModalBill.currency_code} | Original Rate: ₹{settleModalBill.invoice_rate}
            </p>
            <form onSubmit={handleExecuteSettlement}>
              <div className="ot-mc-form-row">
                <label>Foreign Amount to Settle:</label>
                <input
                  type="text"
                  required
                  value={settleInput.foreign_settled_amount}
                  onChange={(e) => setSettleInput({ ...settleInput, foreign_settled_amount: e.target.value })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Settlement Date:</label>
                <input
                  type="date"
                  required
                  value={settleInput.settlement_date}
                  onChange={(e) => setSettleInput({ ...settleInput, settlement_date: e.target.value })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Settlement Rate (INR):</label>
                <input
                  type="text"
                  placeholder="Leave empty to use effective daily rate"
                  value={settleInput.settlement_exchange_rate}
                  onChange={(e) => setSettleInput({ ...settleInput, settlement_exchange_rate: e.target.value })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Bank / Cash Ledger ID:</label>
                <input
                  type="text"
                  required
                  placeholder="Enter Bank or Cash Ledger ID"
                  value={settleInput.bank_or_cash_ledger_id}
                  onChange={(e) => setSettleInput({ ...settleInput, bank_or_cash_ledger_id: e.target.value })}
                />
              </div>
              <div className="ot-mc-form-row">
                <label>Cheque Number (Optional):</label>
                <input
                  type="text"
                  placeholder="6-digit cheque number"
                  value={settleInput.cheque_instrument_number}
                  onChange={(e) => setSettleInput({ ...settleInput, cheque_instrument_number: e.target.value })}
                />
              </div>
              <div className="ot-mc-modal-actions">
                <button type="button" className="ot-btn ot-btn-secondary" onClick={() => setSettleModalBill(null)}>Cancel</button>
                <button type="submit" className="ot-btn ot-btn-primary">Post Settlement & Forex</button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};

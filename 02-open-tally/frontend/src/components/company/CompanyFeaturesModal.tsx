import React, { useState, useEffect, useCallback } from 'react';
import { CompanyFeaturesDto, SaveCompanyFeaturesInput } from '../../types';
import { getCompanyFeatures, saveCompanyFeatures } from '../../services/api';
import './CompanyFeaturesModal.css';

export const INDIAN_GST_STATES: Record<string, string> = {
  '01': 'Jammu and Kashmir',
  '02': 'Himachal Pradesh',
  '03': 'Punjab',
  '04': 'Chandigarh',
  '05': 'Uttarakhand',
  '06': 'Haryana',
  '07': 'Delhi',
  '08': 'Rajasthan',
  '09': 'Uttar Pradesh',
  '10': 'Bihar',
  '11': 'Sikkim',
  '12': 'Arunachal Pradesh',
  '13': 'Nagaland',
  '14': 'Manipur',
  '15': 'Mizoram',
  '16': 'Tripura',
  '17': 'Meghalaya',
  '18': 'Assam',
  '19': 'West Bengal',
  '20': 'Jharkhand',
  '21': 'Odisha',
  '22': 'Chhattisgarh',
  '23': 'Madhya Pradesh',
  '24': 'Gujarat',
  '26': 'Dadra & Nagar Haveli and Daman & Diu',
  '27': 'Maharashtra',
  '29': 'Karnataka',
  '30': 'Goa',
  '31': 'Lakshadweep',
  '32': 'Kerala',
  '33': 'Tamil Nadu',
  '34': 'Puducherry',
  '35': 'Andaman and Nicobar Islands',
  '36': 'Telangana',
  '37': 'Andhra Pradesh',
  '38': 'Ladakh',
  '97': 'Other Territory',
};

const CHARS = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ';

export function calculateGstinCheckDigit(gstin14: string): string | null {
  if (gstin14.length !== 14) return null;
  const upper = gstin14.toUpperCase();
  let sum = 0;
  for (let i = 0; i < 14; i++) {
    const ch = upper[i];
    const val = CHARS.indexOf(ch);
    if (val === -1) return null;
    const factor = i % 2 === 0 ? 1 : 2;
    const prod = val * factor;
    const quotient = Math.floor(prod / 36);
    const remainder = prod % 36;
    sum += quotient + remainder;
  }
  const checkCode = (36 - (sum % 36)) % 36;
  return CHARS[checkCode] || null;
}

export function validateTanFormat(tan: string): { valid: boolean; message: string } {
  const clean = tan.trim().toUpperCase();
  if (!clean) {
    return { valid: false, message: 'TAN is required' };
  }
  if (clean.length !== 10) {
    return { valid: false, message: `TAN must be 10 characters (currently ${clean.length})` };
  }
  const regex = /^[A-Z]{4}[0-9]{5}[A-Z]$/;
  if (!regex.test(clean)) {
    return { valid: false, message: 'Invalid TAN format (e.g. BLRP12345A)' };
  }
  return { valid: true, message: 'Valid TAN' };
}

export function validateGstinFormat(gstin: string): {
  valid: boolean;
  stateCode: string | null;
  stateName: string | null;
  message: string;
} {
  const clean = gstin.trim().toUpperCase();
  if (!clean) {
    return { valid: false, stateCode: null, stateName: null, message: 'GSTIN is required' };
  }
  if (clean.length < 2) {
    return { valid: false, stateCode: null, stateName: null, message: 'GSTIN must have at least 2 characters' };
  }
  const stateCode = clean.substring(0, 2);
  const stateName = INDIAN_GST_STATES[stateCode] || null;
  if (!stateName) {
    return { valid: false, stateCode, stateName: null, message: `Unknown state code '${stateCode}'` };
  }
  if (clean.length !== 15) {
    return {
      valid: false,
      stateCode,
      stateName,
      message: `GSTIN must be 15 characters (currently ${clean.length})`,
    };
  }
  const expectedCheck = calculateGstinCheckDigit(clean.substring(0, 14));
  if (!expectedCheck) {
    return { valid: false, stateCode, stateName, message: 'Invalid characters in GSTIN' };
  }
  if (clean[14] !== expectedCheck) {
    return {
      valid: false,
      stateCode,
      stateName,
      message: `Invalid checksum: expected '${expectedCheck}', got '${clean[14]}'`,
    };
  }
  return { valid: true, stateCode, stateName, message: `Valid GSTIN (${stateName})` };
}

export interface CompanyFeaturesModalProps {
  isOpen: boolean;
  companyId?: string | null;
  companyName?: string;
  isInitialSetup?: boolean;
  onClose: () => void;
  onSaved: (features: CompanyFeaturesDto) => void;
}

export const CompanyFeaturesModal: React.FC<CompanyFeaturesModalProps> = ({
  isOpen,
  companyId,
  companyName,
  isInitialSetup = false,
  onClose,
  onSaved,
}) => {
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Accounting Toggles
  const [billwiseTracking, setBillwiseTracking] = useState(true);

  // Inventory Toggles
  const [maintainInventory, setMaintainInventory] = useState(true);
  const [enablePerpetualCogs, setEnablePerpetualCogs] = useState(true);

  // Taxation (Statutory GST) Toggles
  const [gstEnabled, setGstEnabled] = useState(true);
  const [gstin, setGstin] = useState('29ABCDE1234F1ZW');
  const [stateCode, setStateCode] = useState('29');
  const [registrationType, setRegistrationType] = useState('Regular');

  // Auto-Seeding Toggles
  const [autoSeedGstLedgers, setAutoSeedGstLedgers] = useState(true);
  const [autoSeedInventoryLedgers, setAutoSeedInventoryLedgers] = useState(true);

  // TDS Toggles
  const [tdsEnabled, setTdsEnabled] = useState(false);
  const [tan, setTan] = useState('');
  const [deductorType, setDeductorType] = useState('Company');
  const [autoSeedTdsDefaults, setAutoSeedTdsDefaults] = useState(true);
  const [tanValidation, setTanValidation] = useState<{ valid: boolean; message: string }>({ valid: false, message: '' });

  // Payroll Toggles
  const [enablePayroll, setEnablePayroll] = useState(false);
  const [autoSeedPayrollLedgers, setAutoSeedPayrollLedgers] = useState(true);

  // Validation Status
  const [gstValidation, setGstValidation] = useState(() => validateGstinFormat('29ABCDE1234F1ZW'));

  // Load existing features when opened
  useEffect(() => {
    if (!isOpen) return;
    setError(null);

    const fetchFeatures = async () => {
      setLoading(true);
      try {
        const dto = await getCompanyFeatures(companyId || undefined);
        setMaintainInventory(dto.maintain_inventory);
        setBillwiseTracking(dto.billwise_tracking);
        setEnablePerpetualCogs(dto.enable_perpetual_cogs);
        setGstEnabled(dto.gst_enabled);
        if (dto.gstin) {
          setGstin(dto.gstin);
          setGstValidation(validateGstinFormat(dto.gstin));
        }
        if (dto.state_code) {
          setStateCode(dto.state_code);
        }
        if (dto.registration_type) {
          setRegistrationType(dto.registration_type);
        }
        if (dto.tds_enabled !== undefined) {
          setTdsEnabled(dto.tds_enabled);
        }
        if (dto.tan) {
          setTan(dto.tan);
          setTanValidation(validateTanFormat(dto.tan));
        }
        if (dto.deductor_type) {
          setDeductorType(dto.deductor_type);
        }
        if (dto.enable_payroll !== undefined) {
          setEnablePayroll(dto.enable_payroll);
        }
      } catch (err) {
        // If initial setup or company has no custom settings yet, defaults are preserved
        if (!isInitialSetup) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        setLoading(false);
      }
    };

    fetchFeatures();
  }, [isOpen, companyId, isInitialSetup]);

  // Update GST validation whenever GSTIN changes
  const handleGstinChange = (val: string) => {
    const upper = val.toUpperCase().trim();
    setGstin(upper);
    const result = validateGstinFormat(upper);
    setGstValidation(result);
    if (result.stateCode && INDIAN_GST_STATES[result.stateCode]) {
      setStateCode(result.stateCode);
    }
  };

  const handleSave = useCallback(async () => {
    if (gstEnabled) {
      const val = validateGstinFormat(gstin);
      if (!val.valid) {
        setError(`Cannot save features: ${val.message}`);
        return;
      }
    }

    if (tdsEnabled && tan) {
      const tVal = validateTanFormat(tan);
      if (!tVal.valid) {
        setError(`Cannot save features: ${tVal.message}`);
        return;
      }
    }

    setSaving(true);
    setError(null);

    const input: SaveCompanyFeaturesInput = {
      maintain_inventory: maintainInventory,
      billwise_tracking: billwiseTracking,
      enable_perpetual_cogs: enablePerpetualCogs,
      gst_enabled: gstEnabled,
      gstin: gstEnabled && gstin ? gstin : null,
      state_code: gstEnabled && stateCode ? stateCode : null,
      registration_type: registrationType,
      auto_seed_gst_ledgers: autoSeedGstLedgers,
      auto_seed_inventory_ledgers: autoSeedInventoryLedgers,
      tds_enabled: tdsEnabled,
      tan: tdsEnabled && tan ? tan.trim().toUpperCase() : null,
      deductor_type: tdsEnabled ? deductorType : null,
      auto_seed_tds_defaults: autoSeedTdsDefaults,
      enable_payroll: enablePayroll,
      auto_seed_payroll_ledgers: autoSeedPayrollLedgers,
    };

    try {
      const res = await saveCompanyFeatures(input, companyId || undefined);
      onSaved(res);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }, [
    gstEnabled,
    gstin,
    maintainInventory,
    billwiseTracking,
    enablePerpetualCogs,
    stateCode,
    registrationType,
    autoSeedGstLedgers,
    autoSeedInventoryLedgers,
    companyId,
    onSaved,
    onClose,
    tdsEnabled,
    tan,
    deductorType,
    autoSeedTdsDefaults,
    enablePayroll,
    autoSeedPayrollLedgers,
  ]);

  // Global Keyboard shortcuts: Ctrl+A to save, Esc to close
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && (e.key === 'a' || e.key === 'A')) {
        e.preventDefault();
        e.stopPropagation();
        handleSave();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [isOpen, handleSave, onClose]);

  if (!isOpen) return null;

  return (
    <div className="ot-modal-backdrop" onClick={onClose} data-testid="company-features-modal">
      <div className="ot-modal-content ot-features-modal" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="ot-modal-header ot-features-header">
          <div className="ot-features-header-left">
            <h2>F11: Company Features</h2>
            {companyName && <span className="ot-features-company-name">({companyName})</span>}
          </div>
          <button className="ot-btn-close" onClick={onClose} title="Close (Esc)">
            &times;
          </button>
        </div>

        {/* 2-Step Onboarding Guidance Banner */}
        {isInitialSetup && (
          <div className="ot-features-banner" data-testid="onboarding-guidance-banner">
            <span className="ot-banner-icon">✓</span>
            <div>
              <strong>Company created successfully!</strong>
              <p>Enable features as per your business needs, then press <code>Ctrl+A</code> to accept and land on Gateway.</p>
            </div>
          </div>
        )}

        {error && <div className="ot-modal-error">{error}</div>}

        {loading ? (
          <div className="ot-loading-text">Loading company features...</div>
        ) : (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              handleSave();
            }}
            className="ot-features-body"
          >
            <div className="ot-features-grid">
              {/* Column 1: Accounting & Inventory */}
              <div className="ot-features-column">
                <div className="ot-features-section">
                  <h3 className="ot-section-title">Accounting Features</h3>
                  <div className="ot-features-row">
                    <label>Maintain Accounts</label>
                    <span className="ot-badge ot-badge-yes">Yes (Mandatory)</span>
                  </div>
                  <div className="ot-features-row">
                    <label htmlFor="chk-billwise">Enable Bill-wise entry (for receivables & payables)</label>
                    <select
                      id="chk-billwise"
                      value={billwiseTracking ? 'Yes' : 'No'}
                      onChange={(e) => setBillwiseTracking(e.target.value === 'Yes')}
                    >
                      <option value="Yes">Yes</option>
                      <option value="No">No</option>
                    </select>
                  </div>
                </div>

                <div className="ot-features-section">
                  <h3 className="ot-section-title">Inventory Features</h3>
                  <div className="ot-features-row">
                    <label htmlFor="chk-inventory">Maintain Inventory</label>
                    <select
                      id="chk-inventory"
                      value={maintainInventory ? 'Yes' : 'No'}
                      onChange={(e) => setMaintainInventory(e.target.value === 'Yes')}
                    >
                      <option value="Yes">Yes</option>
                      <option value="No">No</option>
                    </select>
                  </div>
                  {maintainInventory && (
                    <>
                      <div className="ot-features-row">
                        <label htmlFor="chk-perpetual">Integrate Accounts with Inventory (Perpetual COGS)</label>
                        <select
                          id="chk-perpetual"
                          value={enablePerpetualCogs ? 'Yes' : 'No'}
                          onChange={(e) => setEnablePerpetualCogs(e.target.value === 'Yes')}
                        >
                          <option value="Yes">Yes</option>
                          <option value="No">No</option>
                        </select>
                      </div>
                      <div className="ot-features-row ot-features-subrow">
                        <label htmlFor="chk-auto-inv">Auto-seed Inventory Mappings (Stock-in-Hand & COGS)</label>
                        <input
                          id="chk-auto-inv"
                          type="checkbox"
                          checked={autoSeedInventoryLedgers}
                          onChange={(e) => setAutoSeedInventoryLedgers(e.target.checked)}
                        />
                      </div>
                    </>
                  )}
                </div>
              </div>

              {/* Column 2: Taxation (Indian GST) */}
              <div className="ot-features-column">
                <div className="ot-features-section">
                  <h3 className="ot-section-title">Taxation & Statutory Features</h3>
                  <div className="ot-features-row">
                    <label htmlFor="chk-gst">Enable Goods and Services Tax (GST)</label>
                    <select
                      id="chk-gst"
                      value={gstEnabled ? 'Yes' : 'No'}
                      onChange={(e) => setGstEnabled(e.target.value === 'Yes')}
                    >
                      <option value="Yes">Yes</option>
                      <option value="No">No</option>
                    </select>
                  </div>

                  {gstEnabled && (
                    <div className="ot-gst-box">
                      <div className="ot-features-field">
                        <div className="ot-field-header">
                          <label htmlFor="input-gstin">GSTIN / UIN *</label>
                          {gstValidation.valid ? (
                            <span className="ot-badge ot-badge-success" data-testid="gstin-valid-badge">
                              ✓ Valid ({gstValidation.stateName})
                            </span>
                          ) : (
                            <span className="ot-badge ot-badge-danger" data-testid="gstin-invalid-badge">
                              ✗ {gstValidation.message}
                            </span>
                          )}
                        </div>
                        <input
                          id="input-gstin"
                          type="text"
                          maxLength={15}
                          value={gstin}
                          placeholder="e.g. 29ABCDE1234F1ZW"
                          onChange={(e) => handleGstinChange(e.target.value)}
                          className={gstValidation.valid ? 'ot-input-valid' : 'ot-input-invalid'}
                          autoFocus={isInitialSetup}
                        />
                        <span className="ot-field-hint">
                          15-digit Indian GSTIN (State + PAN + Entity + Z + Mod-36 Checksum)
                        </span>
                      </div>

                      <div className="ot-features-row">
                        <label htmlFor="select-state">State Name (State Code)</label>
                        <select
                          id="select-state"
                          value={stateCode}
                          onChange={(e) => setStateCode(e.target.value)}
                        >
                          {Object.entries(INDIAN_GST_STATES).map(([code, name]) => (
                            <option key={code} value={code}>
                              {code} - {name}
                            </option>
                          ))}
                        </select>
                      </div>

                      <div className="ot-features-row">
                        <label htmlFor="select-reg-type">Registration Type</label>
                        <select
                          id="select-reg-type"
                          value={registrationType}
                          onChange={(e) => setRegistrationType(e.target.value)}
                        >
                          <option value="Regular">Regular</option>
                          <option value="Composition">Composition</option>
                        </select>
                      </div>

                      <div className="ot-features-row ot-features-subrow">
                        <label htmlFor="chk-auto-gst">Auto-seed Standard GST Ledgers (8 Ledgers + Mappings)</label>
                        <input
                          id="chk-auto-gst"
                          type="checkbox"
                          checked={autoSeedGstLedgers}
                          onChange={(e) => setAutoSeedGstLedgers(e.target.checked)}
                        />
                      </div>
                    </div>
                  )}

                  {/* TDS Section */}
                  <div className="ot-features-row" style={{ marginTop: '16px' }}>
                    <label htmlFor="chk-tds">Enable Tax Deducted at Source (TDS)</label>
                    <select
                      id="chk-tds"
                      data-testid="select-tds-enabled"
                      value={tdsEnabled ? 'Yes' : 'No'}
                      onChange={(e) => setTdsEnabled(e.target.value === 'Yes')}
                    >
                      <option value="Yes">Yes</option>
                      <option value="No">No</option>
                    </select>
                  </div>

                  {tdsEnabled && (
                    <div className="ot-gst-box" data-testid="tds-config-box">
                      <div className="ot-features-field">
                        <div className="ot-field-header">
                          <label htmlFor="input-tan">Tax Deduction Account Number (TAN) *</label>
                          {tanValidation.valid ? (
                            <span className="ot-badge ot-badge-success" data-testid="tan-valid-badge">
                              ✓ Valid TAN
                            </span>
                          ) : (
                            <span className="ot-badge ot-badge-danger" data-testid="tan-invalid-badge">
                              ✗ {tanValidation.message || 'Required'}
                            </span>
                          )}
                        </div>
                        <input
                          id="input-tan"
                          data-testid="input-tan"
                          type="text"
                          maxLength={10}
                          value={tan}
                          placeholder="e.g. BLRP12345A"
                          onChange={(e) => {
                            const upper = e.target.value.toUpperCase().trim();
                            setTan(upper);
                            setTanValidation(validateTanFormat(upper));
                          }}
                          className={tanValidation.valid ? 'ot-input-valid' : 'ot-input-invalid'}
                        />
                        <span className="ot-field-hint">
                          10-digit Indian TAN (4 Letters + 5 Digits + 1 Letter)
                        </span>
                      </div>

                      <div className="ot-features-row">
                        <label htmlFor="select-deductor-type">Deductor Type</label>
                        <select
                          id="select-deductor-type"
                          data-testid="select-deductor-type"
                          value={deductorType}
                          onChange={(e) => setDeductorType(e.target.value)}
                        >
                          <option value="Company">Company</option>
                          <option value="Individual_HUF">Individual / HUF</option>
                          <option value="Firm">Partnership Firm / LLP</option>
                          <option value="Government">Government</option>
                          <option value="Other">Other</option>
                        </select>
                      </div>

                      <div className="ot-features-row ot-features-subrow">
                        <label htmlFor="chk-auto-tds">Auto-seed Standard TDS Sections (194C, 194J, 194I, etc.)</label>
                        <input
                          id="chk-auto-tds"
                          data-testid="chk-auto-tds"
                          type="checkbox"
                          checked={autoSeedTdsDefaults}
                          onChange={(e) => setAutoSeedTdsDefaults(e.target.checked)}
                        />
                      </div>
                    </div>
                  )}

                  {/* Payroll Section */}
                  <div className="ot-features-row" style={{ marginTop: '16px' }}>
                    <label htmlFor="chk-payroll">Enable Payroll (EPF / ESI / PT)</label>
                    <select
                      id="chk-payroll"
                      data-testid="select-payroll-enabled"
                      value={enablePayroll ? 'Yes' : 'No'}
                      onChange={(e) => setEnablePayroll(e.target.value === 'Yes')}
                    >
                      <option value="Yes">Yes</option>
                      <option value="No">No</option>
                    </select>
                  </div>

                  {enablePayroll && (
                    <div className="ot-features-row ot-features-subrow">
                      <label htmlFor="chk-auto-payroll">Auto-seed Standard 7 Payroll Ledgers & Mappings</label>
                      <input
                        id="chk-auto-payroll"
                        data-testid="chk-auto-payroll"
                        type="checkbox"
                        checked={autoSeedPayrollLedgers}
                        onChange={(e) => setAutoSeedPayrollLedgers(e.target.checked)}
                      />
                    </div>
                  )}
                </div>
              </div>
            </div>

            {/* Modal Actions */}
            <div className="ot-modal-footer ot-features-footer">
              <div className="ot-footer-shortcuts">
                <span><code>Ctrl+A</code>: Accept & Save</span>
                <span><code>Esc</code>: Cancel</span>
              </div>
              <div className="ot-footer-btns">
                <button type="button" className="ot-btn ot-btn-secondary" onClick={onClose} disabled={saving}>
                  Cancel
                </button>
                <button
                  type="submit"
                  className="ot-btn ot-btn-primary"
                  disabled={saving || (gstEnabled && !gstValidation.valid) || (tdsEnabled && tan.length > 0 && !tanValidation.valid)}
                  data-testid="save-features-btn"
                >
                  {saving ? 'Saving Features...' : 'Accept (Ctrl+A)'}
                </button>
              </div>
            </div>
          </form>
        )}
      </div>
    </div>
  );
};

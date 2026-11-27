import { GstCalculationResult, TdsCalculationPreviewResult } from '../../types';
import { minorAmount } from './terminalAmounts';

export function CommercialGstPreview({ preview }: { preview: GstCalculationResult | null }) {
  if (!preview) return null;
  return (
            <div
              data-testid="gst-preview-box"
              style={{
                marginTop: '1.25rem',
                padding: '1rem',
                background: '#f4fbf7',
                border: '1px solid #c2ebd5',
                borderRadius: '6px',
              }}
            >
              <h4 style={{ margin: '0 0 0.5rem 0', color: '#145a32' }}>
                GST Calculation Preview ({preview.supply_type})
              </h4>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))',
                  gap: '0.75rem',
                  fontSize: '0.9rem',
                }}
              >
                <div>
                  <span style={{ color: '#666', display: 'block' }}>Taxable Value:</span>
                  <strong>₹ {preview.taxable_value}</strong>
                </div>
                {preview.supply_type === 'IntraState' ? (
                  <>
                    <div>
                      <span style={{ color: '#666', display: 'block' }}>CGST ({preview.cgst_rate}%):</span>
                      <strong>₹ {preview.cgst_amount}</strong>
                    </div>
                    <div>
                      <span style={{ color: '#666', display: 'block' }}>SGST ({preview.sgst_rate}%):</span>
                      <strong>₹ {preview.sgst_amount}</strong>
                    </div>
                  </>
                ) : (
                  <div>
                    <span style={{ color: '#666', display: 'block' }}>IGST ({preview.igst_rate}%):</span>
                    <strong>₹ {preview.igst_amount}</strong>
                  </div>
                )}
                {minorAmount(preview.cess_amount) > 0n && (
                  <div>
                    <span style={{ color: '#666', display: 'block' }}>Cess ({preview.cess_rate}%):</span>
                    <strong>₹ {preview.cess_amount}</strong>
                  </div>
                )}
                <div>
                  <span style={{ color: '#666', display: 'block' }}>Total Tax:</span>
                  <strong style={{ color: '#b03a2e' }}>₹ {preview.total_tax}</strong>
                </div>
                <div>
                  <span style={{ color: '#666', display: 'block' }}>Grand Total:</span>
                  <strong style={{ color: '#196f3d', fontSize: '1.05rem' }}>₹ {preview.grand_total}</strong>
                </div>
              </div>
            </div>
  );
}

export function CommercialTdsPreview({ preview }: { preview: TdsCalculationPreviewResult | null }) {
  if (!preview) return null;
  return (
            <div
              data-testid="tds-preview-box"
              style={{
                marginTop: '1.25rem',
                padding: '1rem',
                background: '#fcf8e3',
                border: '1px solid #faebcc',
                borderRadius: '6px',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  marginBottom: '0.5rem',
                }}
              >
                <h4 style={{ margin: 0, color: '#8a6d3b' }}>
                  TDS Deduction Preview ({preview.section_code} — {preview.section_name})
                </h4>
                {preview.is_pan_missing_higher_rate && (
                  <span
                    className="ot-badge ot-badge-danger"
                    data-testid="tds-pan-missing-badge"
                    style={{
                      background: '#f8d7da',
                      color: '#721c24',
                      padding: '3px 8px',
                      borderRadius: '4px',
                      fontWeight: 'bold',
                      fontSize: '0.85rem',
                    }}
                  >
                    ⚠️ PAN Missing — 20% Sec 206AA Applied
                  </span>
                )}
              </div>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))',
                  gap: '0.75rem',
                  fontSize: '0.9rem',
                }}
              >
                <div>
                  <span style={{ color: '#666', display: 'block' }}>Taxable Base:</span>
                  <strong>₹ {preview.taxable_base}</strong>
                </div>
                <div>
                  <span style={{ color: '#666', display: 'block' }}>Applicable Rate:</span>
                  <strong>{preview.applicable_rate}%</strong>
                </div>
                <div>
                  <span style={{ color: '#666', display: 'block' }}>TDS Deducted:</span>
                  <strong style={{ color: '#b03a2e' }}>₹ {preview.tds_amount}</strong>
                </div>
                <div>
                  <span style={{ color: '#666', display: 'block' }}>Net Payable:</span>
                  <strong style={{ color: '#196f3d', fontSize: '1.05rem' }}>
                    ₹ {preview.net_payable_amount}
                  </strong>
                </div>
                {!preview.threshold_exceeded && (
                  <div>
                    <span style={{ color: '#666', display: 'block' }}>Threshold:</span>
                    <span style={{ color: '#8a6d3b' }}>Under single/FY limit (₹0)</span>
                  </div>
                )}
              </div>
            </div>
  );
}

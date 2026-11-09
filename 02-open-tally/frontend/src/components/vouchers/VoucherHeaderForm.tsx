import { RefObject } from 'react';
import { VoucherType, VoucherNumberSeries } from '../../types';

interface VoucherHeaderFormProps {
  voucherTypes: VoucherType[];
  selectedTypeId: string;
  onSelectType: (typeId: string) => void;
  seriesList: VoucherNumberSeries[];
  selectedSeriesId: string;
  onSelectSeries: (seriesId: string) => void;
  date: string;
  onChangeDate: (date: string) => void;
  effectiveDate: string;
  onChangeEffectiveDate: (date: string) => void;
  referenceNo: string;
  onChangeReferenceNo: (ref: string) => void;
  referenceDate: string;
  onChangeReferenceDate: (date: string) => void;
  narration: string;
  onChangeNarration: (narration: string) => void;
  provisionalNumber: string;
  dateInputRef?: RefObject<HTMLInputElement>;
}

export function VoucherHeaderForm({
  voucherTypes,
  selectedTypeId,
  onSelectType,
  seriesList,
  selectedSeriesId,
  onSelectSeries,
  date,
  onChangeDate,
  effectiveDate,
  onChangeEffectiveDate,
  referenceNo,
  onChangeReferenceNo,
  referenceDate,
  onChangeReferenceDate,
  narration,
  onChangeNarration,
  provisionalNumber,
  dateInputRef,
}: VoucherHeaderFormProps) {
  return (
    <div className="ot-voucher-header-card">
      <div className="ot-voucher-header-row">
        {/* Voucher Type selection */}
        <div className="ot-form-group ot-type-group">
          <label htmlFor="voucher-type-select">
            Voucher Type <span className="ot-keyboard-hint">[F4-F9]</span>
          </label>
          <select
            id="voucher-type-select"
            className="ot-input"
            value={selectedTypeId}
            onChange={(e) => onSelectType(e.target.value)}
          >
            {voucherTypes.map((vt) => (
              <option key={vt.id} value={vt.id}>
                {vt.name} ({vt.category})
              </option>
            ))}
          </select>
        </div>

        {/* Series selection */}
        <div className="ot-form-group ot-series-group">
          <label htmlFor="voucher-series-select">Numbering Series</label>
          <select
            id="voucher-series-select"
            className="ot-input"
            value={selectedSeriesId}
            onChange={(e) => onSelectSeries(e.target.value)}
          >
            {seriesList.length === 0 ? (
              <option value="">No active series</option>
            ) : (
              seriesList.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.series_name} [{s.series_code}] ({s.numbering_method})
                </option>
              ))
            )}
          </select>
        </div>

        {/* Provisional Voucher Number Display */}
        <div className="ot-form-group ot-vch-num-group">
          <label>Voucher Number</label>
          <div className="ot-provisional-vch-badge">
            <span className="ot-draft-prefix">PROVISIONAL:</span>
            <span className="ot-vch-code">{provisionalNumber || 'DRAFT-NEW'}</span>
          </div>
        </div>

        {/* Voucher Date */}
        <div className="ot-form-group ot-date-group">
          <label htmlFor="voucher-date-input">
            Voucher Date <span className="ot-keyboard-hint">[F2]</span>
          </label>
          <input
            id="voucher-date-input"
            ref={dateInputRef}
            type="date"
            className="ot-input"
            value={date}
            onChange={(e) => onChangeDate(e.target.value)}
          />
        </div>

        {/* Effective Date */}
        <div className="ot-form-group ot-effective-date-group">
          <label htmlFor="voucher-effective-date-input">Effective Date</label>
          <input
            id="voucher-effective-date-input"
            type="date"
            className="ot-input"
            value={effectiveDate}
            onChange={(e) => onChangeEffectiveDate(e.target.value)}
          />
        </div>
      </div>

      <div className="ot-voucher-header-row secondary-row">
        {/* Reference No */}
        <div className="ot-form-group" style={{ flex: 1 }}>
          <label htmlFor="voucher-ref-no">Reference No.</label>
          <input
            id="voucher-ref-no"
            type="text"
            className="ot-input"
            placeholder="e.g. PO-2026-091"
            value={referenceNo}
            onChange={(e) => onChangeReferenceNo(e.target.value)}
          />
        </div>

        {/* Reference Date */}
        <div className="ot-form-group" style={{ flex: 1 }}>
          <label htmlFor="voucher-ref-date">Reference Date</label>
          <input
            id="voucher-ref-date"
            type="date"
            className="ot-input"
            value={referenceDate}
            onChange={(e) => onChangeReferenceDate(e.target.value)}
          />
        </div>

        {/* Narration */}
        <div className="ot-form-group" style={{ flex: 2 }}>
          <label htmlFor="voucher-header-narration">Header Narration</label>
          <input
            id="voucher-header-narration"
            type="text"
            className="ot-input"
            placeholder="Overall voucher narration or remarks"
            value={narration}
            onChange={(e) => onChangeNarration(e.target.value)}
            maxLength={500}
          />
        </div>
      </div>
    </div>
  );
}


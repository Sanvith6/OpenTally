import { Godown, SalesLineRequest, StockItem, Unit, TdsNatureOfPayment } from '../../types';
import { SearchableAccountSelect } from './SearchableAccountSelect';

interface Props {
  line: SalesLineRequest;
  index: number;
  items: StockItem[];
  units: Unit[];
  godowns: Godown[];
  onChange: (field: string, value: string) => void;
  onRemove?: () => void;
}

export function CommercialLineFields({ line, index, items, units, godowns, onChange, onRemove }: Props) {
  return <div className="ot-inventory-fields">
    <input data-terminal-order={100 + index * 10} placeholder="Line description" value={line.description}
      onChange={(event) => onChange('description', event.target.value)} />
    <SearchableAccountSelect label={`Stock item ${index + 1}`} order={101 + index * 10}
      value={line.stock_item_id || ''} options={[{ id: '', name: 'Accounting-only line' }, ...items]}
      onChange={(id) => { onChange('stock_item_id', id); const item = items.find((row) => row.id === id); if (item && !line.description) onChange('description', item.name); }} />
    <select aria-label={`Unit ${index + 1}`} value={line.unit_id || ''} onChange={(event) => onChange('unit_id', event.target.value)}>
      <option value="">Unit</option>{units.map((unit) => <option key={unit.id} value={unit.id}>{unit.symbol}</option>)}
    </select>
    <select aria-label={`Godown ${index + 1}`} value={line.godown_id || ''} onChange={(event) => onChange('godown_id', event.target.value)}>
      <option value="">Godown</option>{godowns.map((godown) => <option key={godown.id} value={godown.id}>{godown.name}</option>)}
    </select>
    <label>Actual quantity<input data-terminal-order={102 + index * 10} aria-label={`Quantity ${index + 1}`}
      value={line.quantity} onChange={(event) => onChange('quantity', event.target.value)} /></label>
    <label>Billed quantity<input data-terminal-order={103 + index * 10} aria-label={`Billed quantity ${index + 1}`}
      value={line.billed_quantity ?? line.quantity} onChange={(event) => onChange('billed_quantity', event.target.value)} /></label>
    <input data-terminal-order={104 + index * 10} aria-label={`Rate ${index + 1}`} value={line.rate} onChange={(event) => onChange('rate', event.target.value)} />
    <select aria-label={`Discount ${index + 1}`} value={line.discount_kind} onChange={(event) => onChange('discount_kind', event.target.value)}>
      <option value="none">No discount</option><option value="amount">Amount</option><option value="percentage">Percentage</option>
    </select>
    <input aria-label={`Discount value ${index + 1}`} value={line.discount_value} onChange={(event) => onChange('discount_value', event.target.value)} />
    {onRemove && <button type="button" onClick={onRemove}>Remove line</button>}
  </div>;
}

export function CommercialTdsSelect({ value, sections, onChange }: { value: string; sections: TdsNatureOfPayment[]; onChange: (value: string) => void }) {
  return <select aria-label="TDS Section" data-testid="select-tds-section" value={value} onChange={(event) => onChange(event.target.value)}>
    <option value="">No TDS Applicable</option>{sections.map((section) => <option key={section.section_code} value={section.section_code}>
      {section.section_code} - {section.name} ({section.rate_individual_huf}%)
    </option>)}
  </select>;
}

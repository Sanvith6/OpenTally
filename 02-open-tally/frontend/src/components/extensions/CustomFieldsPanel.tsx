import { FormEvent, useEffect, useState } from 'react';
import { CreateCustomFieldInput, CustomFieldDefinition, createCustomFieldDefinition, factoryError, listCustomFieldDefinitions } from '../../services/factoryApi';

export function CustomFieldsPanel({ companyId }: { companyId: string }) {
  const [fields, setFields] = useState<CustomFieldDefinition[]>([]);
  const [input, setInput] = useState<CreateCustomFieldInput>({ entity_scope: 'Voucher', field_key: '', label: '', field_type: 'Text', is_required: false, show_on_pdf: false });
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  useEffect(() => { let mounted = true; listCustomFieldDefinitions(companyId).then((rows) => { if (mounted) setFields(rows); }).catch((cause) => { if (mounted) setError(factoryError(cause)); }); return () => { mounted = false; }; }, [companyId]);
  async function save(event: FormEvent) {
    event.preventDefault(); setBusy(true); setError(''); setMessage('');
    try {
      const saved = await createCustomFieldDefinition(companyId, input);
      setFields((old) => [...old, saved]);
      setInput((old) => ({ ...old, field_key: '', label: '' }));
      setMessage(`Custom field ${saved.label || input.label} created.`);
    } catch (cause) { setError(factoryError(cause)); }
    finally { setBusy(false); }
  }
  return <div data-testid="custom-fields-panel"><h3>Custom Fields</h3><p>Independent declarative field definitions. Show on PDF controls inclusion when a document supplies a value.</p>
    <form onSubmit={save} className="ot-report-toolbar">
      <label className="ot-report-field">Entity scope<select value={input.entity_scope} onChange={(e) => setInput((old) => ({ ...old, entity_scope: e.target.value }))}>{['Voucher', 'Ledger', 'StockItem'].map((scope) => <option key={scope}>{scope}</option>)}</select></label>
      <label className="ot-report-field">Field key<input required pattern="[A-Za-z][A-Za-z0-9_]*" value={input.field_key} onChange={(e) => setInput((old) => ({ ...old, field_key: e.target.value }))} /></label>
      <label className="ot-report-field">Field label<input required value={input.label} onChange={(e) => setInput((old) => ({ ...old, label: e.target.value }))} /></label>
      <label className="ot-report-field">Field type<select value={input.field_type} onChange={(e) => setInput((old) => ({ ...old, field_type: e.target.value }))}>{['Text', 'Decimal', 'Date', 'Boolean'].map((type) => <option key={type}>{type}</option>)}</select></label>
      <label className="ot-report-field">Required<input type="checkbox" checked={input.is_required} onChange={(e) => setInput((old) => ({ ...old, is_required: e.target.checked }))} /></label>
      <label className="ot-report-field">Show on PDF<input type="checkbox" checked={input.show_on_pdf} onChange={(e) => setInput((old) => ({ ...old, show_on_pdf: e.target.checked }))} /></label>
      <button disabled={busy} className="ot-report-action" type="submit">Create Custom Field</button>
    </form>{error && <p role="alert" className="ot-report-error">{error}</p>}{message && <p role="status">{message}</p>}
    <table className="ot-report-table"><thead><tr><th>Scope</th><th>Key</th><th>Label</th><th>Type</th><th>Required</th><th>Show on PDF</th></tr></thead><tbody>{fields.map((field) => <tr key={field.id}><td>{field.entity_scope}</td><td>{field.field_key}</td><td>{field.label}</td><td>{field.field_type}</td><td>{field.is_required ? 'Yes' : 'No'}</td><td>{field.show_on_pdf ? 'Yes' : 'No'}</td></tr>)}</tbody></table>{!fields.length && <p className="ot-report-empty">No custom fields defined.</p>}
  </div>;
}

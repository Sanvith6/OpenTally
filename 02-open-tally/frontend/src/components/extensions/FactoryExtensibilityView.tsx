import { useState } from 'react';
import { Company } from '../../types';
import { CustomReportsView } from './CustomReportsView';
import { CustomFieldsPanel } from './CustomFieldsPanel';
import { WebhooksPanel } from './WebhooksPanel';
import { HardwareBridgePanel } from './HardwareBridgePanel';
import '../reports/Reports.css';

interface Props { activeCompany: Company | null; onBack: () => void }
type Tab = 'reports' | 'fields' | 'webhooks' | 'hardware';

export function FactoryExtensibilityView({ activeCompany, onBack }: Props) {
  const [tab, setTab] = useState<Tab>('reports');
  return <section className="ot-report-screen" data-testid="factory-extensibility-view">
    <div className="ot-report-header"><div><h2>Custom Reports &amp; Factory Extensions</h2><p>OpenTally declarative extensions, signed event subscriptions and hardware frame helpers.</p></div><button className="ot-report-back" onClick={onBack}>Back to Gateway</button></div>
    <div className="ot-report-toolbar" role="tablist" aria-label="Factory extensions">{([['reports', 'Report Builder'], ['fields', 'Custom Fields'], ['webhooks', 'Signed Event Webhooks'], ['hardware', 'Weighbridge / Barcode']] as const).map(([value, label]) => <button key={value} className="ot-report-action" aria-pressed={tab === value} onClick={() => setTab(value)}>{label}</button>)}</div>
    {!activeCompany ? <p className="ot-report-empty">Select a company before configuring extensions.</p> : <>
      {tab === 'reports' && <CustomReportsView activeCompany={activeCompany} onBack={onBack} />}
      {tab === 'fields' && <CustomFieldsPanel companyId={activeCompany.id} />}
      {tab === 'webhooks' && <WebhooksPanel companyId={activeCompany.id} />}
      {tab === 'hardware' && <HardwareBridgePanel companyId={activeCompany.id} />}
    </>}
  </section>;
}

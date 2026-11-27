import { FormEvent, useEffect, useState } from 'react';
import { CreateWebhookInput, WebhookSubscription, createWebhookSubscription, factoryError, listWebhookSubscriptions } from '../../services/factoryApi';

export function WebhooksPanel({ companyId }: { companyId: string }) {
  const [subscriptions, setSubscriptions] = useState<WebhookSubscription[]>([]);
  const [input, setInput] = useState<CreateWebhookInput>({ name: '', event_type: 'voucher.posted', target_url: '', hmac_secret: '' });
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  useEffect(() => { let mounted = true; listWebhookSubscriptions(companyId).then((rows) => { if (mounted) setSubscriptions(rows); }).catch((cause) => { if (mounted) setError(factoryError(cause)); }); return () => { mounted = false; }; }, [companyId]);
  async function save(event: FormEvent) {
    event.preventDefault(); setBusy(true); setError(''); setMessage('');
    try {
      const saved = await createWebhookSubscription(companyId, input);
      setSubscriptions((old) => [...old, saved]);
      setInput((old) => ({ ...old, hmac_secret: '' }));
      setMessage('Webhook subscription saved. Signing secret is redacted.');
    } catch (cause) { setError(factoryError(cause)); }
    finally { setBusy(false); }
  }
  return <div data-testid="webhooks-panel"><h3>Signed Event Webhooks</h3><p>Configure an HMAC-SHA256 subscription. Creating a subscription does not itself send a network request or certify delivery.</p>
    <form onSubmit={save} className="ot-report-toolbar">
      <label className="ot-report-field">Webhook name<input required value={input.name} onChange={(e) => setInput((old) => ({ ...old, name: e.target.value }))} /></label>
      <label className="ot-report-field">Event<select value={input.event_type} onChange={(e) => setInput((old) => ({ ...old, event_type: e.target.value }))}>{[['voucher.posted', 'Voucher Posted'], ['voucher.cancelled', 'Voucher Cancelled'], ['stock.moved', 'Stock Moved'], ['jobwork.dispatched', 'Job-Work Dispatched'], ['jobwork.received', 'Job-Work Received']].map(([event, label]) => <option key={event} value={event}>{label}</option>)}</select></label>
      <label className="ot-report-field">Webhook URL<input required type="url" placeholder="https://example.test/events" value={input.target_url} onChange={(e) => setInput((old) => ({ ...old, target_url: e.target.value }))} /></label>
      <label className="ot-report-field">Signing secret<input required type="password" autoComplete="new-password" value={input.hmac_secret} onChange={(e) => setInput((old) => ({ ...old, hmac_secret: e.target.value }))} /></label>
      <button disabled={busy} className="ot-report-action" type="submit">Create Webhook</button>
    </form>{error && <p className="ot-report-error" role="alert">{error}</p>}{message && <p role="status">{message}</p>}
    <table className="ot-report-table"><thead><tr><th>Name</th><th>Event</th><th>Target</th><th>Signing Secret</th><th>Status</th></tr></thead><tbody>{subscriptions.map((subscription) => <tr key={subscription.id}><td>{subscription.name}</td><td>{subscription.event_type}</td><td style={{ overflowWrap: 'anywhere' }}>{subscription.target_url}</td><td><span title="Secret is deliberately omitted from the subscription response">Configured · redacted</span></td><td>{subscription.is_active ? 'Active' : 'Inactive'}</td></tr>)}</tbody></table>{!subscriptions.length && <p className="ot-report-empty">No webhook subscriptions configured.</p>}
  </div>;
}

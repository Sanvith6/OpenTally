import { useState, useEffect, useRef } from 'react';
import { Company, AuthSessionInfo } from '../../types';
import { authenticateCompanyUser } from '../../services/api';
import '../vouchers/BillWiseAllocationModal.css';

interface Props {
  company: Company;
  onSuccess: (session: AuthSessionInfo) => void;
  onCancel: () => void;
}

export function CompanyLoginModal({ company, onSuccess, onCancel }: Props) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const usernameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    usernameRef.current?.focus();
  }, []);

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username.trim()) {
      setError('Username is required');
      return;
    }
    if (!password) {
      setError('Password is required');
      return;
    }

    try {
      setSubmitting(true);
      setError(null);
      const session = await authenticateCompanyUser({
        company_id: company.id,
        username: username.trim(),
        password,
      });
      onSuccess(session);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Invalid username or password');
    } finally {
      setSubmitting(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      onCancel();
    }
  };

  return (
    <div className="billwise-modal-overlay" onKeyDown={handleKeyDown}>
      <div className="billwise-modal-container" style={{ maxWidth: '440px', padding: '24px' }}>
        <div style={{ borderBottom: '2px solid #004d40', paddingBottom: '8px', marginBottom: '16px' }}>
          <h2 style={{ margin: 0, fontSize: '18px', color: '#004d40', fontWeight: 'bold' }}>
            Company Login (Security Control)
          </h2>
          <div style={{ fontSize: '12px', color: '#555', marginTop: '4px' }}>
            Company: <strong style={{ color: '#00251a' }}>{company.name}</strong>
          </div>
        </div>

        {error && (
          <div
            style={{
              padding: '10px',
              backgroundColor: '#ffebee',
              color: '#c62828',
              borderRadius: '4px',
              marginBottom: '14px',
              fontSize: '13px',
              fontWeight: 500,
            }}
          >
            {error}
          </div>
        )}

        <form onSubmit={handleLogin}>
          <div style={{ marginBottom: '14px' }}>
            <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, color: '#333', marginBottom: '4px' }}>
              Name of User
            </label>
            <input
              ref={usernameRef}
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="Enter username"
              disabled={submitting}
              style={{
                width: '100%',
                padding: '8px 10px',
                fontSize: '14px',
                border: '1px solid #ccc',
                borderRadius: '4px',
                outline: 'none',
                boxSizing: 'border-box',
              }}
            />
          </div>

          <div style={{ marginBottom: '20px' }}>
            <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, color: '#333', marginBottom: '4px' }}>
              Password
            </label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Enter password"
              disabled={submitting}
              style={{
                width: '100%',
                padding: '8px 10px',
                fontSize: '14px',
                border: '1px solid #ccc',
                borderRadius: '4px',
                outline: 'none',
                boxSizing: 'border-box',
              }}
            />
          </div>

          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px' }}>
            <button
              type="button"
              onClick={onCancel}
              disabled={submitting}
              style={{
                padding: '8px 16px',
                backgroundColor: '#f5f5f5',
                color: '#333',
                border: '1px solid #ccc',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '13px',
              }}
            >
              Cancel (Esc)
            </button>
            <button
              type="submit"
              disabled={submitting}
              style={{
                padding: '8px 20px',
                backgroundColor: '#004d40',
                color: '#fff',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '13px',
                fontWeight: 600,
              }}
            >
              {submitting ? 'Logging in...' : 'Login (Enter)'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

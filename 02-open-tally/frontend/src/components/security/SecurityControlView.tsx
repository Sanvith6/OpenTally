import { useCallback, useEffect, useState } from 'react';
import { Company, CompanyUser, SecurityStatusDto, UserRole, AuthSessionInfo } from '../../types';
import {
  createCompanyUser,
  deactivateCompanyUser,
  enableCompanySecurity,
  getCompanySecurityStatus,
  listCompanyUsers,
  updateCompanyUserPassword,
  updateCompanyUserRole,
} from '../../services/api';
import '../reports/Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
  currentUser?: AuthSessionInfo | null;
}

export function SecurityControlView({ activeCompany, onBack, currentUser: _currentUser }: Props) {
  const [status, setStatus] = useState<SecurityStatusDto | null>(null);
  const [users, setUsers] = useState<CompanyUser[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  // Enable Security Form State
  const [enableAdminUser, setEnableAdminUser] = useState('admin');
  const [enableAdminPass, setEnableAdminPass] = useState('');

  // Add User Form State
  const [showAddUser, setShowAddUser] = useState(false);
  const [newUsername, setNewUsername] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [newRole, setNewRole] = useState<UserRole>('DataEntry');

  // Change Password Modal State
  const [pwdModalUser, setPwdModalUser] = useState<CompanyUser | null>(null);
  const [changePwdValue, setChangePwdValue] = useState('');

  const loadSecurityData = useCallback(async () => {
    if (!activeCompany) return;
    try {
      setLoading(true);
      setError(null);
      const st = await getCompanySecurityStatus(activeCompany.id);
      setStatus(st);
      if (st.security_enabled) {
        const uList = await listCompanyUsers(activeCompany.id);
        setUsers(uList);
      } else {
        setUsers([]);
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to load security status');
    } finally {
      setLoading(false);
    }
  }, [activeCompany]);

  useEffect(() => {
    loadSecurityData();
  }, [loadSecurityData]);

  const handleEnableSecurity = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    if (!enableAdminUser.trim()) {
      setError('Admin username is required');
      return;
    }
    if (!enableAdminPass || enableAdminPass.length < 4) {
      setError('Password must be at least 4 characters');
      return;
    }

    try {
      setLoading(true);
      setError(null);
      await enableCompanySecurity({
        company_id: activeCompany.id,
        admin_username: enableAdminUser.trim(),
        admin_password: enableAdminPass,
      });
      setMessage('Security control enabled successfully. Administrator account created.');
      setEnableAdminPass('');
      await loadSecurityData();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to enable security');
    } finally {
      setLoading(false);
    }
  };

  const handleCreateUser = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    if (!newUsername.trim()) {
      setError('Username is required');
      return;
    }
    if (!newPassword || newPassword.length < 4) {
      setError('Password must be at least 4 characters');
      return;
    }

    try {
      setLoading(true);
      setError(null);
      await createCompanyUser({
        company_id: activeCompany.id,
        username: newUsername.trim(),
        password: newPassword,
        role: newRole,
      });
      setMessage(`User '${newUsername.trim()}' created successfully as ${newRole}.`);
      setNewUsername('');
      setNewPassword('');
      setShowAddUser(false);
      await loadSecurityData();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to create user');
    } finally {
      setLoading(false);
    }
  };

  const handleRoleChange = async (user: CompanyUser, roleStr: string) => {
    if (!activeCompany) return;
    const role = roleStr as UserRole;
    try {
      setLoading(true);
      setError(null);
      await updateCompanyUserRole({
        company_id: activeCompany.id,
        user_id: user.id,
        role,
      });
      setMessage(`Updated role for '${user.username}' to ${role}.`);
      await loadSecurityData();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to update user role');
    } finally {
      setLoading(false);
    }
  };

  const handleDeactivate = async (user: CompanyUser) => {
    if (!activeCompany) return;
    if (!confirm(`Are you sure you want to deactivate user '${user.username}'?`)) return;

    try {
      setLoading(true);
      setError(null);
      await deactivateCompanyUser({
        company_id: activeCompany.id,
        user_id: user.id,
      });
      setMessage(`User '${user.username}' deactivated.`);
      await loadSecurityData();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to deactivate user');
    } finally {
      setLoading(false);
    }
  };

  const handleChangePassword = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany || !pwdModalUser) return;
    if (!changePwdValue || changePwdValue.length < 4) {
      setError('Password must be at least 4 characters');
      return;
    }

    try {
      setLoading(true);
      setError(null);
      await updateCompanyUserPassword({
        company_id: activeCompany.id,
        user_id: pwdModalUser.id,
        new_password: changePwdValue,
      });
      setMessage(`Password updated for user '${pwdModalUser.username}'.`);
      setPwdModalUser(null);
      setChangePwdValue('');
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to update password');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="report-container" style={{ padding: '20px', maxWidth: '1000px', margin: '0 auto' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <div>
          <h1 style={{ margin: 0, fontSize: '20px', color: '#004d40', fontWeight: 'bold' }}>
            Users and Passwords (Company Security Control)
          </h1>
          <div style={{ fontSize: '13px', color: '#666', marginTop: '4px' }}>
            Company: <strong>{activeCompany?.name || 'No Company Selected'}</strong>
          </div>
        </div>
        <button
          onClick={onBack}
          style={{
            padding: '6px 14px',
            backgroundColor: '#f5f5f5',
            border: '1px solid #ccc',
            borderRadius: '4px',
            cursor: 'pointer',
            fontSize: '13px',
          }}
        >
          Back to Gateway (Esc)
        </button>
      </div>

      {message && (
        <div style={{ padding: '10px 14px', backgroundColor: '#e8f5e9', color: '#2e7d32', borderRadius: '4px', marginBottom: '16px', fontSize: '13px' }}>
          {message}
        </div>
      )}

      {error && (
        <div style={{ padding: '10px 14px', backgroundColor: '#ffebee', color: '#c62828', borderRadius: '4px', marginBottom: '16px', fontSize: '13px' }}>
          {error}
        </div>
      )}

      {/* Security Status Box */}
      <div style={{ backgroundColor: '#fff', border: '1px solid #ddd', borderRadius: '6px', padding: '18px', marginBottom: '20px', boxShadow: '0 1px 3px rgba(0,0,0,0.05)' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <div>
            <span style={{ fontSize: '14px', fontWeight: 600, color: '#333' }}>Security Status: </span>
            <span
              style={{
                fontSize: '13px',
                fontWeight: 'bold',
                padding: '3px 8px',
                borderRadius: '4px',
                backgroundColor: status?.security_enabled ? '#e8f5e9' : '#fff3e0',
                color: status?.security_enabled ? '#2e7d32' : '#e65100',
              }}
            >
              {status?.security_enabled ? 'ENABLED' : 'DISABLED'}
            </span>
          </div>

          {status?.active_user && (
            <div style={{ fontSize: '13px', color: '#004d40' }}>
              Current User: <strong>{status.active_user.username}</strong> ({status.active_user.role})
            </div>
          )}
        </div>

        {!status?.security_enabled && (
          <div style={{ marginTop: '16px', borderTop: '1px solid #eee', paddingTop: '16px' }}>
            <h3 style={{ margin: '0 0 10px 0', fontSize: '15px', color: '#004d40' }}>Enable Company Security</h3>
            <p style={{ fontSize: '13px', color: '#555', margin: '0 0 14px 0' }}>
              Enabling security control will mandate user logins for this company and enforce role-based access control (Admin, Accountant, DataEntry, Auditor) across vouchers, reports, and MCA audit trails.
            </p>
            <form onSubmit={handleEnableSecurity} style={{ display: 'flex', gap: '12px', alignItems: 'flex-end', flexWrap: 'wrap' }}>
              <div>
                <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, marginBottom: '4px' }}>Admin Username</label>
                <input
                  type="text"
                  value={enableAdminUser}
                  onChange={(e) => setEnableAdminUser(e.target.value)}
                  placeholder="admin"
                  style={{ padding: '6px 10px', fontSize: '13px', border: '1px solid #ccc', borderRadius: '4px', width: '180px' }}
                />
              </div>
              <div>
                <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, marginBottom: '4px' }}>Password</label>
                <input
                  type="password"
                  value={enableAdminPass}
                  onChange={(e) => setEnableAdminPass(e.target.value)}
                  placeholder="Min 4 characters"
                  style={{ padding: '6px 10px', fontSize: '13px', border: '1px solid #ccc', borderRadius: '4px', width: '200px' }}
                />
              </div>
              <button
                type="submit"
                disabled={loading}
                style={{
                  padding: '7px 18px',
                  backgroundColor: '#004d40',
                  color: '#fff',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: 'pointer',
                  fontWeight: 600,
                  fontSize: '13px',
                }}
              >
                {loading ? 'Enabling...' : 'Enable Security Control'}
              </button>
            </form>
          </div>
        )}
      </div>

      {/* User Management Section */}
      {status?.security_enabled && (
        <div style={{ backgroundColor: '#fff', border: '1px solid #ddd', borderRadius: '6px', padding: '18px', boxShadow: '0 1px 3px rgba(0,0,0,0.05)' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '14px' }}>
            <h2 style={{ margin: 0, fontSize: '16px', color: '#004d40', fontWeight: 'bold' }}>Company Users (RBAC)</h2>
            <button
              onClick={() => setShowAddUser(!showAddUser)}
              style={{
                padding: '6px 14px',
                backgroundColor: showAddUser ? '#757575' : '#004d40',
                color: '#fff',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '13px',
                fontWeight: 600,
              }}
            >
              {showAddUser ? 'Cancel' : '+ Add User (Alt+A)'}
            </button>
          </div>

          {/* Add User Form */}
          {showAddUser && (
            <div style={{ backgroundColor: '#f9f9f9', border: '1px solid #e0e0e0', borderRadius: '4px', padding: '14px', marginBottom: '16px' }}>
              <h3 style={{ margin: '0 0 10px 0', fontSize: '14px', color: '#333' }}>Create New User</h3>
              <form onSubmit={handleCreateUser} style={{ display: 'flex', gap: '12px', alignItems: 'flex-end', flexWrap: 'wrap' }}>
                <div>
                  <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, marginBottom: '4px' }}>Username</label>
                  <input
                    type="text"
                    value={newUsername}
                    onChange={(e) => setNewUsername(e.target.value)}
                    placeholder="e.g. accountant1"
                    style={{ padding: '6px 10px', fontSize: '13px', border: '1px solid #ccc', borderRadius: '4px', width: '160px' }}
                  />
                </div>
                <div>
                  <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, marginBottom: '4px' }}>Password</label>
                  <input
                    type="password"
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                    placeholder="Min 4 chars"
                    style={{ padding: '6px 10px', fontSize: '13px', border: '1px solid #ccc', borderRadius: '4px', width: '160px' }}
                  />
                </div>
                <div>
                  <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, marginBottom: '4px' }}>Role</label>
                  <select
                    value={newRole}
                    onChange={(e) => setNewRole(e.target.value as UserRole)}
                    style={{ padding: '6px 10px', fontSize: '13px', border: '1px solid #ccc', borderRadius: '4px', width: '140px' }}
                  >
                    <option value="Admin">Admin</option>
                    <option value="Accountant">Accountant</option>
                    <option value="DataEntry">DataEntry</option>
                    <option value="Auditor">Auditor</option>
                  </select>
                </div>
                <button
                  type="submit"
                  disabled={loading}
                  style={{
                    padding: '7px 18px',
                    backgroundColor: '#004d40',
                    color: '#fff',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: 'pointer',
                    fontWeight: 600,
                    fontSize: '13px',
                  }}
                >
                  Save User
                </button>
              </form>
            </div>
          )}

          {/* User Table */}
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '13px' }}>
            <thead>
              <tr style={{ backgroundColor: '#f5f5f5', borderBottom: '2px solid #ddd', textAlign: 'left' }}>
                <th style={{ padding: '8px 10px' }}>Username</th>
                <th style={{ padding: '8px 10px' }}>Role</th>
                <th style={{ padding: '8px 10px' }}>Status</th>
                <th style={{ padding: '8px 10px' }}>Created Date</th>
                <th style={{ padding: '8px 10px' }}>Last Login</th>
                <th style={{ padding: '8px 10px', textAlign: 'right' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => (
                <tr key={u.id} style={{ borderBottom: '1px solid #eee' }}>
                  <td style={{ padding: '8px 10px', fontWeight: 600, color: '#333' }}>{u.username}</td>
                  <td style={{ padding: '8px 10px' }}>
                    <select
                      value={u.role}
                      disabled={u.is_active === 0 || loading}
                      onChange={(e) => handleRoleChange(u, e.target.value)}
                      style={{ padding: '3px 6px', fontSize: '12px', border: '1px solid #ccc', borderRadius: '3px' }}
                    >
                      <option value="Admin">Admin</option>
                      <option value="Accountant">Accountant</option>
                      <option value="DataEntry">DataEntry</option>
                      <option value="Auditor">Auditor</option>
                    </select>
                  </td>
                  <td style={{ padding: '8px 10px' }}>
                    <span
                      style={{
                        padding: '2px 6px',
                        borderRadius: '3px',
                        fontSize: '11px',
                        fontWeight: 600,
                        backgroundColor: u.is_active === 1 ? '#e8f5e9' : '#ffebee',
                        color: u.is_active === 1 ? '#2e7d32' : '#c62828',
                      }}
                    >
                      {u.is_active === 1 ? 'Active' : 'Inactive'}
                    </span>
                  </td>
                  <td style={{ padding: '8px 10px', color: '#666' }}>{u.created_at.slice(0, 10)}</td>
                  <td style={{ padding: '8px 10px', color: '#666' }}>
                    {u.last_login_at ? u.last_login_at.slice(0, 19).replace('T', ' ') : 'Never'}
                  </td>
                  <td style={{ padding: '8px 10px', textAlign: 'right' }}>
                    {u.is_active === 1 && (
                      <div style={{ display: 'inline-flex', gap: '6px' }}>
                        <button
                          onClick={() => setPwdModalUser(u)}
                          style={{
                            padding: '3px 8px',
                            backgroundColor: '#e0f2f1',
                            color: '#004d40',
                            border: '1px solid #b2dfdb',
                            borderRadius: '3px',
                            cursor: 'pointer',
                            fontSize: '11px',
                          }}
                        >
                          Change Pwd
                        </button>
                        <button
                          onClick={() => handleDeactivate(u)}
                          style={{
                            padding: '3px 8px',
                            backgroundColor: '#ffebee',
                            color: '#c62828',
                            border: '1px solid #ffcdd2',
                            borderRadius: '3px',
                            cursor: 'pointer',
                            fontSize: '11px',
                          }}
                        >
                          Deactivate
                        </button>
                      </div>
                    )}
                  </td>
                </tr>
              ))}
              {users.length === 0 && (
                <tr>
                  <td colSpan={6} style={{ textAlign: 'center', padding: '16px', color: '#888' }}>
                    No users found for this company.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* Change Password Modal */}
      {pwdModalUser && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: 'rgba(0,0,0,0.5)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
        >
          <div style={{ backgroundColor: '#fff', borderRadius: '6px', padding: '20px', width: '360px', boxShadow: '0 4px 12px rgba(0,0,0,0.2)' }}>
            <h3 style={{ margin: '0 0 10px 0', fontSize: '16px', color: '#004d40' }}>
              Change Password: {pwdModalUser.username}
            </h3>
            <form onSubmit={handleChangePassword}>
              <div style={{ marginBottom: '16px' }}>
                <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, marginBottom: '4px' }}>New Password</label>
                <input
                  type="password"
                  value={changePwdValue}
                  onChange={(e) => setChangePwdValue(e.target.value)}
                  placeholder="Min 4 characters"
                  autoFocus
                  style={{ width: '100%', padding: '8px 10px', fontSize: '13px', border: '1px solid #ccc', borderRadius: '4px', boxSizing: 'border-box' }}
                />
              </div>
              <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
                <button
                  type="button"
                  onClick={() => setPwdModalUser(null)}
                  style={{ padding: '6px 12px', backgroundColor: '#f5f5f5', border: '1px solid #ccc', borderRadius: '4px', cursor: 'pointer', fontSize: '12px' }}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={loading}
                  style={{ padding: '6px 16px', backgroundColor: '#004d40', color: '#fff', border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '12px', fontWeight: 600 }}
                >
                  Update
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}

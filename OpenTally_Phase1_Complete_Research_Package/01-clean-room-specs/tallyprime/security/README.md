# Workstream 8: Security & Audit Specification

## 1. Authentication & Role-Based Access Control (RBAC)
TallyPrime implements application-level security managed through Company Alteration (`Alt + K > Security`):
1. **Security Activation**: When `Control User Access to Company Data = Yes`, an Administrator username and password are required.
2. **Predefined Roles**:
   - `Owner`: Unrestricted administrative rights. Can create users, configure security, split data, and alter all masters/vouchers.
   - `Data Entry`: Operational role restricted from altering masters, viewing sensitive financial ratios, or splitting data.
3. **Granular Permissions Matrix**:
   Permissions can be configured to `Disallow` or `Allow` combinations of:
   - Actions: `Create`, `Alter`, `Display`, `Print`, `Preview`.
   - Resource Types: `Accounting Masters`, `Inventory Masters`, `Vouchers`, `Financial Reports`.

## 2. Backdated Voucher Restrictions
- An administrator can set: `Cut-off date for Backdated vouchers` (e.g., `31-Oct-2024`).
- Any non-admin user attempting to save or alter a voucher with a transaction date prior to or on the cut-off date is blocked with: *"Backdated vouchers are disallowed for this user role."*

## 3. TallyPrime Edit Log (MCA Audit Trail Compliance)
- **Statutory Mandate**: Effective 1-April-2023, under Rule 3(1) of the Companies (Accounts) Rules, 2014, Indian companies must use accounting software featuring an immutable audit trail.
- **Edit Log Features**:
  1. **Permanent Enablement**: In TallyPrime Edit Log edition, the edit log cannot be disabled by any user.
  2. **Tracked Entities**: Every creation, alteration, and cancellation of:
     - All Voucher types.
     - All Ledger masters.
     - All Stock Item masters.
     - Company configuration.
  3. **Logged Metadata**:
     - Version Number (increments from 1).
     - Date and Time of alteration.
     - Username of user performing the action.
     - Detailed field-level diff (old value vs new value).

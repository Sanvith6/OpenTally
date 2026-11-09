import sqlite3
import re

def normalize_sql(sql: str) -> str:
    # Collapse whitespace and strip
    return re.sub(r'\s+', ' ', sql).strip()

conn = sqlite3.connect(":memory:")
conn.execute("CREATE TABLE companies (id TEXT);")
conn.execute("CREATE TABLE ledgers (id TEXT, company_id TEXT);")
conn.execute("CREATE TABLE voucher_lines (id TEXT, company_id TEXT, ledger_id TEXT);")

sql1 = """CREATE TRIGGER trg_prevent_ledger_delete_with_vouchers
BEFORE DELETE ON ledgers
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
  AND EXISTS (SELECT 1 FROM voucher_lines WHERE company_id = OLD.company_id AND ledger_id = OLD.id)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete ledger: it is referenced in existing voucher lines.');
END;"""

conn.execute(sql1)
stored_before = conn.execute("SELECT sql FROM sqlite_master WHERE type='trigger' AND name='trg_prevent_ledger_delete_with_vouchers'").fetchone()[0]

# Now drop and recreate with IF NOT EXISTS
conn.execute("DROP TRIGGER trg_prevent_ledger_delete_with_vouchers;")
sql2 = """CREATE TRIGGER IF NOT EXISTS trg_prevent_ledger_delete_with_vouchers
BEFORE DELETE ON ledgers
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
  AND EXISTS (SELECT 1 FROM voucher_lines WHERE company_id = OLD.company_id AND ledger_id = OLD.id)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete ledger: it is referenced in existing voucher lines.');
END;"""
conn.execute(sql2)
stored_after = conn.execute("SELECT sql FROM sqlite_master WHERE type='trigger' AND name='trg_prevent_ledger_delete_with_vouchers'").fetchone()[0]

print("BEFORE:", stored_before)
print("AFTER: ", stored_after)
print("EXACT EQUAL:", stored_before == stored_after)
print("NORMALIZED EQUAL:", normalize_sql(stored_before) == normalize_sql(stored_after))

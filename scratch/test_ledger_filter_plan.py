import sqlite3

con = sqlite3.connect(':memory:')
cur = con.cursor()
cur.execute('''
CREATE TABLE vouchers (
    company_id TEXT NOT NULL,
    id TEXT NOT NULL,
    date TEXT NOT NULL,
    PRIMARY KEY (company_id, id)
)
''')
cur.execute('''
CREATE TABLE voucher_lines (
    company_id TEXT NOT NULL,
    id TEXT NOT NULL,
    voucher_id TEXT NOT NULL,
    ledger_id TEXT NOT NULL,
    PRIMARY KEY (company_id, id)
)
''')
cur.execute('CREATE INDEX idx_voucher_lines_ledger ON voucher_lines(company_id, ledger_id)')

plan1 = cur.execute('''
EXPLAIN QUERY PLAN
SELECT v.* FROM vouchers v
WHERE v.company_id = ?
  AND EXISTS (
      SELECT 1 FROM voucher_lines vl 
      WHERE vl.company_id = v.company_id 
        AND vl.ledger_id = ? 
        AND vl.voucher_id = v.id
  )
''', ('c1', 'l1')).fetchall()

print('EXISTS plan:', plan1)

plan2 = cur.execute('''
EXPLAIN QUERY PLAN
SELECT v.* FROM vouchers v
WHERE v.company_id = ?
  AND v.id IN (
      SELECT vl.voucher_id FROM voucher_lines vl 
      WHERE vl.company_id = ? 
        AND vl.ledger_id = ?
  )
''', ('c1', 'c1', 'l1')).fetchall()

print('IN plan:', plan2)

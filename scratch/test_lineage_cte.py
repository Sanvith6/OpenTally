import sqlite3

con = sqlite3.connect(':memory:')
cur = con.cursor()
cur.execute('''
CREATE TABLE vouchers (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    reversal_of_voucher_id TEXT,
    date TEXT,
    sequence_number INTEGER
)
''')

cur.execute("INSERT INTO vouchers VALUES ('v1', 'c1', NULL, '2026-05-01', 1)")
cur.execute("INSERT INTO vouchers VALUES ('v2', 'c1', 'v1', '2026-05-02', 2)")
cur.execute("INSERT INTO vouchers VALUES ('v3', 'c1', NULL, '2026-05-03', 3)")
cur.execute("INSERT INTO vouchers VALUES ('v_other', 'c2', 'v1', '2026-05-02', 1)")

query = '''
WITH RECURSIVE voucher_lineage(id, company_id, parent_id, depth, path) AS (
    -- Anchor: The target voucher
    SELECT 
        v.id, 
        v.company_id, 
        v.reversal_of_voucher_id AS parent_id, 
        0 AS depth, 
        '/' || v.id || '/' AS path
    FROM vouchers v
    WHERE v.company_id = ? AND v.id = ?

    UNION ALL

    -- Backward traversal (Finding what was reversed)
    SELECT 
        parent.id, 
        parent.company_id, 
        parent.reversal_of_voucher_id, 
        vl.depth + 1, 
        vl.path || parent.id || '/'
    FROM vouchers parent
    JOIN voucher_lineage vl ON parent.company_id = vl.company_id AND parent.id = vl.parent_id
    WHERE vl.depth < 10 
      AND instr(vl.path, '/' || parent.id || '/') = 0

    UNION ALL

    -- Forward traversal (Finding what reversed this voucher)
    SELECT 
        child.id, 
        child.company_id, 
        child.reversal_of_voucher_id, 
        vl.depth + 1, 
        vl.path || child.id || '/'
    FROM vouchers child
    JOIN voucher_lineage vl ON child.company_id = vl.company_id AND child.reversal_of_voucher_id = vl.id
    WHERE vl.depth < 10 
      AND instr(vl.path, '/' || child.id || '/') = 0
)
SELECT DISTINCT v.*
FROM voucher_lineage vl
JOIN vouchers v ON vl.company_id = v.company_id AND vl.id = v.id
ORDER BY v.date ASC, v.sequence_number ASC, v.id ASC;
'''

res = cur.execute(query, ('c1', 'v2')).fetchall()
print('Traversing from v2:', res)

res2 = cur.execute(query, ('c1', 'v1')).fetchall()
print('Traversing from v1:', res2)

import sqlite3

con = sqlite3.connect(':memory:')

def test_expr(expr, tests):
    sql = f"SELECT :val, CASE WHEN ({expr}) THEN 'PASS' ELSE 'FAIL' END"
    results = {}
    for val, expected in tests.items():
        res = con.execute(sql, {'val': val}).fetchone()[1]
        results[val] = (res, expected, res == expected)
    return results

# Test SQLite GLOB negation
print("Testing GLOB with [!0-9]:", con.execute("SELECT '123' NOT GLOB '*[!0-9]*'").fetchone()[0])
print("Testing GLOB with [^0-9]:", con.execute("SELECT '123' NOT GLOB '*[^0-9]*'").fetchone()[0])


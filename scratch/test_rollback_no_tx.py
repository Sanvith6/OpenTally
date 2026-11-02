import sqlite3
con = sqlite3.connect(':memory:')
try:
    con.execute("ROLLBACK;")
    print("Rollback succeeded when no transaction active")
except sqlite3.OperationalError as e:
    print("Rollback failed as expected:", e)


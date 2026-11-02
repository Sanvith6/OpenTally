import sqlite3
con = sqlite3.connect(':memory:')
print("123 GLOB '*[!0-9]*':", con.execute("SELECT '123' GLOB '*[!0-9]*'").fetchone()[0])
print("123 GLOB '*[^0-9]*':", con.execute("SELECT '123' GLOB '*[^0-9]*'").fetchone()[0])
print("12a GLOB '*[!0-9]*':", con.execute("SELECT '12a' GLOB '*[!0-9]*'").fetchone()[0])
print("12a GLOB '*[^0-9]*':", con.execute("SELECT '12a' GLOB '*[^0-9]*'").fetchone()[0])
print("12^ GLOB '*[^0-9]*':", con.execute("SELECT '12^' GLOB '*[^0-9]*'").fetchone()[0])


import sqlite3

con = sqlite3.connect(':memory:')

vals = ["01.00", "00.50", "0001.25", "0.10", "0.50", "0.0001", "1.00"]
for v in vals:
    res = con.execute("SELECT :v GLOB '0.*'", {'v': v}).fetchone()[0]
    print(f"{v} GLOB '0.*' -> {res}")


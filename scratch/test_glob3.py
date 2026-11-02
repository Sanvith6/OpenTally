import sqlite3
con = sqlite3.connect(':memory:')

chars = ['0', '5', '9', 'a', 'Z', '.', '-', '+', '^', '!', ' ']
for c in chars:
    val = f"x{c}y"
    g1 = con.execute("SELECT :v GLOB '*[^0-9]*'", {'v': val}).fetchone()[0]
    g2 = con.execute("SELECT :v GLOB '*[!0-9]*'", {'v': val}).fetchone()[0]
    print(f"char '{c}': [^0-9]={g1}, [!0-9]={g2}")


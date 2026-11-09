import os, re

def search_repo():
    print("Searching for queries on vouchers and voucher_lines...")
    base_dir = r"02-open-tally/src-tauri/src"
    patterns = [
        re.compile(r"SELECT\s+.*?FROM\s+vouchers\b.*?(?=\")", re.DOTALL | re.IGNORECASE),
        re.compile(r"SELECT\s+.*?FROM\s+voucher_lines\b.*?(?=\")", re.DOTALL | re.IGNORECASE),
    ]
    for root, _, files in os.walk(base_dir):
        for f in files:
            if f.endswith(".rs"):
                path = os.path.join(root, f)
                content = open(path, encoding="utf-8", errors="ignore").read()
                for pat in patterns:
                    for m in pat.finditer(content):
                        query = " ".join(m.group(0).split())
                        print(f"[{f}]: {query[:140]}...")

search_repo()


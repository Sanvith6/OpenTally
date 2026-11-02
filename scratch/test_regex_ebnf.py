import re

pattern = re.compile(r"^(?:[1-9]\d*(?:\.\d{1,4})?|0\.(?=\d{1,4}$)\d*[1-9]\d*)$")

accepted = [
    "0.10", "0.50", "0.0001", "1", "1.0", "1.00", "1.000", "1.0000", "10.00", "10.25",
    "0.01", "0.001", "0.9999", "10", "10000.25", "999999999999.9999", "100.5"
]

rejected = [
    "0", "0.0", "0.00", "01.00", "00.50", "0001.25", "010.50", ".50", "1.", "1.2.3",
    "10.00001", "0.00001", "-10.00", "+10.00", "", " ", "  ", "\t", "10.00 ", " 10.00",
    "00", "00.00", "0.000", "0.0000", "00.000", "0.00000", "1abc", "abc", "000", "01",
    "007", "10. ", "1.0.0", "1..00"
]

for val in accepted:
    assert pattern.match(val), f"Regex failed on accepted val: {val}"

for val in rejected:
    assert not pattern.match(val), f"Regex matched rejected val: {val}"

print("Regex matches all accepted and rejects all rejected!")


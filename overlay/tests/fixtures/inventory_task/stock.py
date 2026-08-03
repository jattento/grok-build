"""Inventory helpers."""

import csv
from pathlib import Path

DATA = Path(__file__).parent / "inventory.csv"


def load_rows():
    with DATA.open() as fh:
        return list(csv.reader(fh))


def total_value():
    """Sum of qty * unit_price over the inventory, rounded to 2 decimals."""
    total = 0.0
    for row in load_rows():
        total += int(row[1]) * float(row[2])
    return round(total, 2)


def low_stock(threshold=5):
    """SKUs whose qty is strictly below `threshold`, in file order."""
    out = []
    for row in load_rows():
        if int(row[1]) < threshold:
            out.append(row[0])
    return out

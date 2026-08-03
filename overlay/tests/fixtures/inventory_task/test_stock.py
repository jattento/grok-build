from stock import low_stock, total_value


def test_total_value():
    assert total_value() == 490.72


def test_low_stock():
    assert low_stock() == ["A-100", "C-300", "E-500"]


def test_low_stock_threshold():
    assert low_stock(3) == ["C-300", "E-500"]

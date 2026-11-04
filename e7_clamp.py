def clamp(i, lo, hi):
    """Clamp ``i`` to the inclusive range ``[lo, hi]``.

    Requires ``lo <= hi``; invalid ranges raise ``ValueError``.
    """
    if lo > hi:
        raise ValueError("lo must not be greater than hi")
    result = max(lo, min(i, hi))
    assert lo <= result <= hi
    return result


if __name__ == "__main__":
    for value in range(-3, 4):
        for lower in range(-3, 4):
            for upper in range(-3, 4):
                if lower > upper:
                    try:
                        clamp(value, lower, upper)
                    except ValueError:
                        pass
                    else:
                        raise AssertionError("invalid range was accepted")
                else:
                    assert clamp(value, lower, upper) in (value, lower, upper)

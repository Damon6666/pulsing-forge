def clamp(value, lower, upper):
    """Clamp value to the inclusive range [lower, upper]."""
    if lower > upper:
        raise ValueError("lower must be less than or equal to upper")
    return max(lower, min(value, upper))

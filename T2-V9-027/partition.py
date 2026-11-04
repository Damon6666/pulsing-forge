from typing import Dict, List, Tuple


def partition_1_to_120(num_chunks: int) -> List[Tuple[int, int]]:
    """Partition 1..120 into exactly num_chunks non-empty inclusive ranges.

    num_chunks must be from 1 through 120: values above 120 cannot produce
    the required number of non-empty contiguous integer ranges.
    """
    if isinstance(num_chunks, bool):
        raise TypeError("num_chunks must be an int, not bool")
    if type(num_chunks) is not int:
        raise TypeError("num_chunks must be an int")
    if not 1 <= num_chunks <= 120:
        raise ValueError("num_chunks must be in 1..120")

    base_size, remainder = divmod(120, num_chunks)
    chunks: List[Tuple[int, int]] = []
    start = 1

    for index in range(num_chunks):
        extra = 1 if index < remainder else 0
        end = start + base_size + extra - 1
        chunks.append((start, end))
        start = end + 1

    return chunks


def sum_squares(start: int, end: int) -> int:
    """Return the sum of squares for a non-empty inclusive range."""
    if isinstance(start, bool) or isinstance(end, bool):
        raise TypeError("start and end must be ints, not bool")
    if type(start) is not int or type(end) is not int:
        raise TypeError("start and end must be ints")
    if not 1 <= start <= end <= 120:
        raise ValueError("range must satisfy 1 <= start <= end <= 120")

    return sum(number * number for number in range(start, end + 1))


def process_chunks(num_chunks: int) -> Dict[str, int]:
    """Compute all chunk results and their grand total."""
    results: Dict[str, int] = {}
    for index, (start, end) in enumerate(partition_1_to_120(num_chunks), 1):
        results[f"chunk_{index}"] = sum_squares(start, end)
    results["TOTAL"] = sum(results.values())
    return results

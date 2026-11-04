import unittest

from partition import partition_1_to_120, process_chunks


class PartitionTests(unittest.TestCase):
    def test_chunks_cover_range_without_overlap(self):
        chunks = partition_1_to_120(7)
        self.assertEqual(len(chunks), 7)
        self.assertEqual(chunks[0][0], 1)
        self.assertEqual(chunks[-1][-1], 120)
        self.assertTrue(all(start <= end for start, end in chunks))
        self.assertTrue(all(right[0] > left[1] for left, right in zip(chunks, chunks[1:])))

    def test_all_numbers_can_be_singletons(self):
        self.assertEqual(partition_1_to_120(120), [(number, number) for number in range(1, 121)])

    def test_total_matches_direct_sum(self):
        self.assertEqual(process_chunks(7)["TOTAL"], sum(n * n for n in range(1, 121)))

    def test_bool_is_explicitly_rejected(self):
        with self.assertRaises(TypeError):
            partition_1_to_120(True)
        with self.assertRaises(TypeError):
            partition_1_to_120(False)

    def test_non_int_and_out_of_range_are_rejected(self):
        with self.assertRaises(TypeError):
            partition_1_to_120(3.0)
        with self.assertRaises(ValueError):
            partition_1_to_120(0)
        with self.assertRaises(ValueError):
            partition_1_to_120(121)


if __name__ == "__main__":
    unittest.main()

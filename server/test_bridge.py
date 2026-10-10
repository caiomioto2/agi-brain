import unittest
import read_only_bridge as bridge


class BridgeTests(unittest.TestCase):
    def test_only_allowlisted_reads(self):
        for operation in ["update", "delete", "sql", "get_credentials", "shell"]:
            with self.assertRaises(ValueError):
                bridge.query_for({"op": operation})
        self.assertIn("deleted_at IS NULL", bridge.query_for({"op": "graph"}))
        self.assertIn("NOT s.archived", bridge.query_for({"op": "graph"}))

    def test_identity_is_quoted_and_bounded(self):
        self.assertEqual(bridge.literal("a'; DELETE FROM pages; --"), "'a''; DELETE FROM pages; --'")
        for value in [None, 12, "x\x00", "x" * 1001]:
            with self.assertRaises(ValueError):
                bridge.literal(value)
        for keys in [[], [{}] * 101, ["bad"]]:
            with self.assertRaises(ValueError):
                bridge.query_for({"op": "pages", "keys": keys})


if __name__ == "__main__":
    unittest.main()

import unittest

from scripts import ols_lsmr_reuse_development as reuse


class LsmrInventoryTests(unittest.TestCase):
    def test_pool_widths_have_distinct_identities_and_commands(self):
        scopes = [s for s in reuse.scopes() if s["design"] == "mixed_three"
                  and s["k"] == 29 and s["threads"] == 1
                  and s["memory"] == "low" and not s["guard"]]
        self.assertEqual({s["poolsize"] for s in scopes}, {2, 10})
        self.assertEqual(len({s["id"] for s in scopes}), 2)
        for scope in scopes:
            self.assertIn(f'poolsize({scope["poolsize"]})', reuse.command(scope))
            self.assertNotIn("poolsize(", reuse.command(scope, reference=True))

    def test_tail_correctness_case_is_separate_from_timing_targets(self):
        scopes = reuse.scopes()
        self.assertTrue(any(s["k"] == 10 and s["memory"] == "low" for s in scopes))
        for scope in scopes:
            if scope["k"] == 29 and scope["memory"] == "low":
                self.assertEqual((scope["k"] + 1) % scope["poolsize"], 0)


if __name__ == "__main__":
    unittest.main()

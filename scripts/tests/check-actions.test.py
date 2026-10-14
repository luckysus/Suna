"""Public GitHub HTML fixtures; no credentials or live Actions mutation."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("suna_actions", Path(__file__).resolve().parents[1] / "check-actions.py")
actions = importlib.util.module_from_spec(spec)
spec.loader.exec_module(actions)
SHA = "a" * 40


def page(state, sha=SHA, color="color-fg-success"):
    return f'<div class="Box-row {color}"><a href="/luckysus/Suna/commit/{sha}">commit</a><a href="/luckysus/Suna/actions/runs/123" aria-label="{state}: workflow">workflow</a></div>'


class PublicStateTests(unittest.TestCase):
    def test_current_github_completed_successfully_label_is_success(self):
        with patch.object(actions, "read", return_value=page("completed successfully")):
            self.assertEqual(actions.inspect("luckysus/Suna", SHA)[0]["state"], "success")

    def test_running_and_cancelled_never_become_success(self):
        for state, expected in [("currently running", "in_progress"), ("cancelled", "cancelled")]:
            with patch.object(actions, "read", return_value=page(state)):
                self.assertEqual(actions.inspect("luckysus/Suna", SHA)[0]["state"], expected)

    def test_failure_keeps_the_actual_failed_job_step(self):
        pages = [page("failed", color="color-fg-danger"), '<a href="/luckysus/Suna/actions/runs/123/job/456">job</a>', '<check-step data-name="Run deterministic Rust suite" data-conclusion="failure">']
        with patch.object(actions, "read", side_effect=pages):
            run = actions.inspect("luckysus/Suna", SHA)[0]
            self.assertEqual(run["state"], "failed")
            self.assertEqual(run["failedSteps"], ["Run deterministic Rust suite"])

    def test_different_sha_is_not_accepted(self):
        with patch.object(actions, "read", return_value=page("completed successfully", "b" * 40)):
            self.assertEqual(actions.inspect("luckysus/Suna", SHA), [])


if __name__ == "__main__":
    unittest.main()

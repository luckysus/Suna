"""Public GitHub HTML fixtures; no credentials or live Actions mutation."""
import importlib.util
import json
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
    def test_react_statuses_require_explicit_success_conclusion(self):
        for state,expected in [
            ({'status':'WORKFLOW_RUN_STATUS_IN_PROGRESS'},'in_progress'),
            ({'status':'WORKFLOW_RUN_STATUS_QUEUED'},'pending'),
            ({'status':'WORKFLOW_RUN_STATUS_COMPLETED','conclusion':'WORKFLOW_RUN_CONCLUSION_SUCCESS'},'success'),
            ({'status':'WORKFLOW_RUN_STATUS_COMPLETED','conclusion':'WORKFLOW_RUN_CONCLUSION_FAILURE'},'failed'),
            ({'status':'WORKFLOW_RUN_STATUS_COMPLETED','conclusion':'WORKFLOW_RUN_CONCLUSION_TIMED_OUT'},'failed'),
            ({'status':'WORKFLOW_RUN_STATUS_COMPLETED','conclusion':'WORKFLOW_RUN_CONCLUSION_CANCELLED'},'cancelled'),
            ({'status':'WORKFLOW_RUN_STATUS_COMPLETED'},'unverified'),
            ({'status':'future_state'},'unverified')]:
            row={'description':{'linkUrl':'/luckysus/Suna/commit/'+SHA},'runUrl':'https://github.com/luckysus/Suna/actions/runs/123','state':state}
            html='<script type="application/json">'+json.dumps({'payload':{'allWorkflowRunsRoute':{'workflowRuns':[row]}}})+'</script>'
            with self.subTest(state=state),patch.object(actions,'read',return_value=html):
                self.assertEqual(actions.inspect('luckysus/Suna',SHA)[0]['state'],expected)

    def test_malformed_foreign_and_other_commit_react_rows_cannot_report_green(self):
        valid={'description':{'linkUrl':'/luckysus/Suna/commit/'+SHA},'runUrl':'https://github.com/luckysus/Suna/actions/runs/123','state':{'status':'WORKFLOW_RUN_STATUS_COMPLETED','conclusion':'WORKFLOW_RUN_CONCLUSION_SUCCESS'}}
        for row in [None,{}, {**valid,'description':None},{**valid,'runUrl':'https://example.invalid/123'},
                    {**valid,'description':{'linkUrl':'/luckysus/Suna/commit/'+'b'*40}}]:
            html='<script type="application/json">'+json.dumps({'workflowRuns':[row]})+'</script>'
            self.assertEqual(actions.embedded_runs(html,'luckysus/Suna',SHA),[])
        self.assertEqual(actions.embedded_runs('<script type="application/json">{broken</script>','luckysus/Suna',SHA),[])

    def test_all_jobs_and_complete_job_step_are_required(self):
        run={'url':'https://github.com/luckysus/Suna/actions/runs/123','state':'success'}
        job='<a href="/luckysus/Suna/actions/runs/123/job/456">job</a>'
        for steps,expected in [('',False),('<check-step data-name="Rust" data-conclusion="success">',False),
            ('<check-step data-name="Rust" data-conclusion="failure"><check-step data-name="Complete job" data-conclusion="success">',False),
            ('<check-step data-name="Rust" data-conclusion="null"><check-step data-name="Complete job" data-conclusion="success">',False),
            ('<check-step data-name="Rust" data-conclusion="success"><check-step data-name="Complete job" data-conclusion="success">',True)]:
            with patch.object(actions,'read',side_effect=[job,steps]):
                self.assertEqual(actions.complete_job_evidence(run)['allJobStepsSuccess'],expected)
        with patch.object(actions,'read',return_value='no jobs'):
            self.assertFalse(actions.complete_job_evidence(run)['allJobStepsSuccess'])
        with patch.object(actions,'read',side_effect=[job+job.replace('456','789'),'<check-step data-name="Complete job" data-conclusion="success">','']):
            self.assertFalse(actions.complete_job_evidence(run)['allJobStepsSuccess'])

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

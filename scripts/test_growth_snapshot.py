"""Deterministic accounting checks, with no GitHub requests."""
from datetime import datetime, timezone
import unittest
from unittest.mock import patch
from urllib.error import URLError
import tempfile
from pathlib import Path
from growth_snapshot import build_snapshot, pages, main

NOW = datetime(2026, 9, 5, tzinfo=timezone.utc)
META = {"stargazers_count": 12, "forks_count": 2, "open_issues_count": 3}


def asset(id, name, downloads):
    return {"id": id, "name": name, "download_count": downloads, "browser_download_url": "https://example.com/asset", "created_at": "2026-09-01T00:00:00Z"}


def snapshot(assets, previous=None, issues=()):
    return build_snapshot("owner/repo", META, [{"tag_name": "v1", "assets": assets}], issues, NOW, previous)


class AccountingTests(unittest.TestCase):
    def test_checksums_excluded_and_first_observation_unknown(self):
        result = snapshot([asset(1, "app.zip", 10), asset(2, "app.zip.sha256", 400)])
        self.assertEqual(result["archive_downloads_total"], 10)
        self.assertEqual(len(result["assets"]), 1)
        self.assertIsNone(result["assets"][0]["downloads_change"])

    def test_asset_ids_handle_replacements_and_removals(self):
        previous = snapshot([asset(1, "app.zip", 10), asset(2, "old.dmg", 20)])
        result = snapshot([asset(1, "renamed.zip", 15), asset(3, "old.dmg", 3)], previous)
        self.assertEqual([a["downloads_change"] for a in result["assets"]], [5, None])
        self.assertEqual(result["missing_previous_asset_ids"], [2])

    def test_decreasing_download_counter_is_unknown(self):
        previous = snapshot([asset(1, "app.zip", 10)])
        result = snapshot([asset(1, "app.zip", 2)], previous)
        self.assertIsNone(result["assets"][0]["downloads_change"])

    def test_issue_window_filters_old_issues_and_prs(self):
        def issue(number, created, **extra):
            return {"number": number, "created_at": created, "title": "Test", "html_url": "https://example.com", "state": "closed", **extra}
        result = snapshot([], issues=[issue(1, "2026-09-01T00:00:00Z"), issue(2, "2026-08-01T00:00:00Z"), issue(3, "2026-09-02T00:00:00Z", pull_request={})])
        self.assertEqual([i["number"] for i in result["new_issues"]], [1])

    def test_repository_mismatch_rejected(self):
        with self.assertRaises(ValueError):
            snapshot([], {"repository": "other/repo"})

    def test_star_loss_preserved(self):
        previous = snapshot([])
        previous["counters"]["stars"] = 15
        self.assertEqual(snapshot([], previous)["counter_changes"]["stars"], -3)


class RequestTests(unittest.TestCase):
    @patch("growth_snapshot.request_json")
    def test_pagination_keeps_existing_query(self, request):
        request.side_effect = [list(range(100)), [100]]
        self.assertEqual(list(pages("/issues?state=all")), list(range(101)))
        self.assertEqual(request.call_args_list[1].args[0], "/issues?state=all&per_page=100&page=2")

    @patch("growth_snapshot.request_json")
    def test_pagination_failure_does_not_return_partial_result(self, request):
        request.side_effect = [list(range(100)), URLError("offline")]
        with self.assertRaises(URLError):
            list(pages("/releases"))

    @patch("growth_snapshot.request_json", side_effect=URLError("offline"))
    def test_request_failure_preserves_previous_output(self, request):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "snapshot.json"
            output.write_text("existing snapshot")
            with patch("sys.argv", ["growth_snapshot.py", "--output", str(output)]), patch("sys.stderr"):
                self.assertEqual(main(), 1)
            self.assertEqual(output.read_text(), "existing snapshot")


if __name__ == "__main__":
    unittest.main()

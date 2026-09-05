# Vinny acquisition and usage checks

Use this kit to help people reach their first working connection and decide whether Vinny earns a place in their workflow.

- [Launch drafts and checklist](launch.md)
- [Real demo recording plan](demo.md)
- [Ten-person tester protocol](testing.md)
- [Blank tester tracker](testers.csv)

## Weekly public GitHub snapshot

Run from the repository root with Python 3.10+:

```bash
python3 scripts/growth_snapshot.py --output /tmp/vinny-growth/2026-09-07.json
python3 scripts/growth_snapshot.py --previous /tmp/vinny-growth/2026-09-07.json --output /tmp/vinny-growth/2026-09-14.json
```

These example dates are filenames, not claimed observations. The first run creates a baseline. A public snapshot taken on 2026-09-05 is included as [the initial baseline](baseline-2026-09-05.json) and can also be passed to `--previous`. Later runs compare against the exact prior file. Keep snapshots in a durable private location or use the workflow artifacts described below. Do not commit private traffic data or tester responses.

Public counters work without credentials, subject to GitHub API limits. To include repository visits and clones, supply `GH_TOKEN` or `GITHUB_TOKEN` with the required repository access and add `--traffic`. Missing traffic access is recorded as unavailable, not zero. A public-data request failure stops the run and preserves the previous output file.

The **Growth snapshot** Actions workflow runs on Sundays at 23:00 UTC and can be run manually. It downloads the latest successful run's snapshot as the baseline and uploads a new `vinny-growth-snapshot` artifact retained for 90 days. The first run in `sarimabbas/vinny` compares with the committed public baseline. Forks start without a comparison. A failure to retrieve a known prior snapshot fails the workflow rather than silently resetting the baseline. The workflow runs after this change reaches the default branch and Actions is enabled. GitHub may delay scheduled jobs.

The JSON reports:

- Stars and forks, with net changes since the prior capture.
- Per-asset download counters and changes, matched by GitHub asset ID.
- Downloadable application archives, excluding checksum files and GitHub-generated source archives.
- Removed asset IDs and unknown deltas for new or reset assets.
- Issues created in the past seven days, excluding pull requests.
- Optional rolling-window visitor and clone data.

Asset downloads include repeated downloads, prereleases, and automation. They are not installations or active users. New asset downloads are deliberately not counted as a measured interval delta without a prior observation. The open-issue counter includes pull requests and is labelled accordingly. Overlapping traffic windows and daily unique visitors must not be added together.

## Monday review

Record the observation window, any release or distribution activity, and links to relevant issues. Then answer:

1. Did stars, downloadable-asset counts, or first-connection reports change?
2. Which installation or connection failures appeared more than once?
3. Did testers return seven days after their first connection?
4. What is the single most useful fix or next distribution experiment?

Compare similar windows. A launch spike alone cannot establish that a particular post caused downloads or retained use. The script intentionally adds no app telemetry.

## Website and search measurement

Before starting a campaign, verify that the website is deployed at its intended public domain. Add that property to Google Search Console using an account that controls the domain, verify ownership, and submit the deployed sitemap if available. Domain ownership and account access cannot be inferred from this repository.

For distinct distribution links, use a stable convention such as `?utm_source=hackernews&utm_medium=community&utm_campaign=vinny_first_connection`. Keep a small campaign log of URL, date, audience, and destination. Tagged URLs alone do not collect measurements. To measure visits or download clicks, first choose and configure a website analytics service or use hosting request logs, document what is collected, and test an event end to end. No analytics account or tracking identifier is invented by this change.

Until that is connected, report website visitors, click-through, and conversion rates as **unavailable**. Use the GitHub snapshot and voluntary tester cohort for the measurements that can actually be obtained.

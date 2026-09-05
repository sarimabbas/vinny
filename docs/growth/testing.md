# First ten testers

Recruit ten people who agree to try Vinny and receive one follow-up seven days after their first connection. Use anonymous tester IDs in [testers.csv](testers.csv). Keep contact information and detailed responses in a separate private location. The CSV in the repository must remain a blank template.

## Initial test

Ask each person to start with the public README and the published build, rather than a private walkthrough. Record:

1. macOS version, chip architecture, Vinny version, and viewer name/version.
2. Intended task and how they previously handled it.
3. Whether installation and permissions completed.
4. Whether they established a remote connection with the documented secure setup.
5. Approximate minutes to first connection, the step that blocked them, and whether you helped.
6. Whether the selected display appeared and the intended control or view-only behavior worked.

A successful first connection means the viewer on another machine displayed the selected Mac desktop using the documented setup. A download, launched app, or open port does not count. Mark assisted connections separately. Do not collect passwords, network addresses, screen contents, or personal files.

Useful prompt:

> Please try Vinny using the README. Let me know whether you reached your Mac's desktop from the other machine, where you got stuck, and what you wanted to use it for. You can stop at any point. Please leave credentials and private screen contents out of your report.

## Seven-day follow-up

Send only to testers who agreed to a follow-up:

> Have you used Vinny again since your first connection? If so, what did you use it for? If not, what got in the way or what did you use instead?

Count repeat use only when the tester reports a separate later session. Report results as counts with denominators, such as `4 of 7 responding activated testers used it again`. Keep nonresponses visible and do not count them as confirmed non-use. Do not infer population retention from a small recruited cohort.

## Review

Fix repeated blockers before broadening distribution. Preserve the original attempt outcome even when a later attempt succeeds. Review assisted and unassisted activation separately, and connect each actionable failure to an issue without posting private tester details.

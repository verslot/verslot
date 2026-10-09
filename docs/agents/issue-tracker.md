# Issue tracker: GitHub

Issues and specs live in GitHub Issues for verslot/verslot.
GitHub Issues is the only backlog for new work.

Use the gh CLI from this repository:
- Create: gh issue create --title "..." --body-file <file>
- Read: gh issue view <number> --json number,title,body,labels,comments
- List: gh issue list --state open
- Comment: gh issue comment <number> --body-file <file>
- Apply or remove labels: gh issue edit <number> --add-label "..."
  or --remove-label "..."
- Close: gh issue close <number>

When a skill says to publish to the issue tracker, create a GitHub issue.
When a skill says to fetch a ticket, read the corresponding GitHub issue.

PRs as a request surface: no.

Historical milestone task lists are frozen reference material,
not the current backlog.

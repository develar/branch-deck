# Proposal

## Why

An update of `youtrack` started from the Setup dot ran in the main window, which showed nothing of it; Settings → Tools,
opened a moment later, had a page of its own that knew nothing of the action and offered "Update to 0.8.2" again, and
each click was refused: "Cannot update youtrack — youtrack is busy with another action". The action that runs was the
state of the page that started it, while Rust refused a second one: the app refused what it offered itself.

Settings showed the paths without `~` (its window had no `core:path` permission), the line of the brd skill was a cut
list of SKILL.md files, and `teamcity` on a server whose sign-in failed offered two sign-ins of that server: Sign In
Again on its line and the form below, filled with the same server.

## What Changes

- Rust keeps the actions of the CLIs that run (`cli_tools_running`, the event `cli-tools-running`, a version that grows
  with each change); every window shows one that another window started, or that ran before its page loaded, with its
  last line and Cancel. The progress channel of `run_cli_tool_action` goes: the event carries the lines. A second
  action is `cli_tool_busy`, which toasts nothing; a guard frees the tool also when the action panics.
- An item of the Setup dot of a CLI whose action runs shows its last line instead of its buttons.
- Settings has `core:path`: the paths are under `~`.
- Tools is one list with dividers; IntelliJ IDEA says "Not installed"; the brd skill line names the skill directories
  of the agents (`AgentCliStatus.skill_dirs`), as the lines of the other skills, and "Agents" opens that page.
- A server whose sign-in failed says why in `text-warning`, the page of the reason a link (`CliToolServer.error_url`).
- `teamcity` signs in to a server it knows from the head of its row, with Other Server… for the form; the form shows by
  itself only for a server the CLI does not know.

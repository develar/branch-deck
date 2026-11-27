# Proposal

## Why

A running Safe Push of the whole stack refuses the Safe Push of each branch of it (`robot_running`), and the robot of a
branch refuses the whole stack. The only way past was `--cancel-running`, which cancels the long robot. To land a cheap
piece beside it, the user had to start a robot with `safePush.cmd` outside of Branch Deck.

## What Changes

- `brd safe-push --keep-running` starts a Safe Push beside a running Safe Push of another subject that lands the same
  commits (`Descendant`, `Ancestor`) and cancels nothing. Both robots run; the one that merges second rebases onto the
  first or fails in Patronus. A robot on the last commit still refuses.
- With it, the start pushes the branches that robot holds by its span. A push of them fails no robot. A push into
  the branch a robot runs on stays refused.
- It conflicts with `--cancel-running`, `--patronus-dry-run` and `--create-pull-request`.
- The `robot_running` refusal of such a robot names it in its message and has the start with it as a next step that
  needs the approval of the user.
- `LandingAllowances::keep_running` carries it into `domain_policy::land::decide`. The app sets it never and keeps the
  wait.

Capabilities: `cli/agent-cli`, `branch/whole-stack`.

/// `bd help agent`: the workflow of an agent that groups commits into virtual branches (docs/guidelines/agent-cli.md)
pub const AGENT_HELP: &str = "\
bd: Branch Deck for agents. JSON on stdout (every object has \"schema\": 1), logs on stderr (-v for debug).

Branch Deck groups the commits ahead of the baseline (e.g. origin/master) into virtual branches by the
`(name)` prefix of their subjects, or by an issue key (`ABC-123 Fix ...`). A sync rebuilds each group as
`{prefix}/virtual/{name}` on the baseline; the working tree is never touched.

Global options: -C <repo>, --prefix <p> (else the app setting, else `git config branchdeck.branchPrefix`),
--full-ids (ids are abbreviated by default), -v.

Errors: {\"schema\":1,\"error\":{\"code\",\"message\",\"commits\",\"retryable\"}}.
Exit codes: 0 ok, 1 conflict, 2 invalid input, 3 stale (read again and retry), 4 refused, 5 git or network error.
`bd schema` prints the JSON Schema of every output.

Workflow:
1. `bd status` for the branches and the unassigned commits, then `bd commits --files`, and
   `bd commits --patch --branch <b>` for the clusters that are unclear.
2. Cluster the commits by intent. `dependencies` ({from, to, files}: `from` touches lines `to` added) and
   `cluster_overlap` (files two branches both change) show which commits can't be separated: a commit
   goes with, or stacks on, the commits it depends on.
3. `bd check < plan.json` until it is clean.                     (not yet available)
4. `bd group --expect-head <id> < plan.json`, then `bd sync`.    (not yet available)
5. `bd describe <branch> --title ... --body ...` for each branch.  (not yet available)
6. Show the result to the user. `bd push` only after the user approves. (not yet available)

Rules: never reorder, squash or drop commits; never touch the working tree. Until `bd group` exists, ask
the user to apply the grouping in the Branch Deck app.
";

## ADDED Requirements

### Requirement: The dialog: The default agent comes first

The agent picker MUST offer "Default Agent" first, before the agents of the IDE, also when the IDE lists none. Its
launch MUST name no target, so the IDE starts the default of its New Session button; it has no model, mode and Plan of
its own.

#### Scenario: The default agent stands first
- **WHEN** the IDE lists its agents, or none
- **THEN** the agent picker offers "Default Agent" first
  [@test] src/lib/air-launch.test.ts (`stands first before the agents of the IDE, also when the IDE lists none`)

#### Scenario: The launch of the default agent
- **WHEN** the user starts the default agent
- **THEN** the launch names no target, no model, no effort, no mode and no Plan
  [@test] src/lib/air-launch.test.ts (`sends no target, so the IDE starts the default of its New Session button`)

## MODIFIED Requirements

### Requirement: The dialog: The pickers start from the choice remembered for the repository

The pickers MUST start from the choice remembered for the repository while the IDE still offers it, else the first
agent (the default agent in the dialog), its default model, the default effort, the default mode, Plan off and Run. A
choice remembered by an older version has no mode and no Plan.

#### Scenario: Nothing remembered
- **WHEN** the dialog opens with nothing remembered for the repository
- **THEN** the pickers start with the first agent, its default model, the default effort and Run
- **AND** in the dialog the first agent is the default agent, which also comes back as the remembered choice
  [@test] src/lib/air-launch.test.ts (`starts with the first agent, its default model, the default effort and Run`, `is the first choice with nothing remembered, and comes back as the remembered choice`)

#### Scenario: The remembered agent, model, effort and start
- **WHEN** the IDE still offers the remembered agent, model and effort
- **THEN** the pickers start with them and the remembered start
  [@test] src/lib/air-launch.test.ts (`starts with the remembered agent, model, effort and start while the IDE still offers them`)

#### Scenario: A remembered model or effort the agent no longer offers
- **WHEN** the agent no longer offers the remembered model or effort
- **THEN** the pickers drop it
  [@test] src/lib/air-launch.test.ts (`drops a remembered model or effort the agent no longer offers`)

#### Scenario: The remembered agent is gone
- **WHEN** the IDE no longer offers the remembered agent
- **THEN** the pickers take the first agent and keep the remembered start
  [@test] src/lib/air-launch.test.ts (`takes the first agent when the remembered one is gone, and keeps the remembered start`)

#### Scenario: No agents
- **WHEN** the list of the pickers has no agents
- **THEN** the dialog has no form of pickers
- **AND** the dialog never gets such a list: the default agent is always in it
  [@test] src/lib/air-launch.test.ts (`has no model and no effort for an agent without models, and no form without agents`, `stands first before the agents of the IDE, also when the IDE lists none`)

#### Scenario: The remembered mode, Plan and plan effort
- **WHEN** the IDE still offers the remembered mode and Plan
- **THEN** the pickers start with the remembered mode, Plan and plan effort
- **AND** without them in the IDE, the default mode and Plan off
- **AND** a choice remembered by an older version starts with no mode and no Plan
  [@test] src/lib/air-launch.test.ts (`starts with the remembered mode, Plan and plan effort while the IDE still offers them`)

### Requirement: The dialog: A launch without a session id says it started

A draft has no session to follow: a toast MUST say "Opened a draft for Codex in Air", with the title of the session
and the IDE.

#### Scenario: A draft is accepted without a session id
- **WHEN** the IDE opens a draft
- **THEN** a toast says "Opened a draft for Codex in Air"

### Requirement: The dialog: A refusal stays in the dialog in its words

A refusal MUST stay in the dialog in its words, the message of the IDE only for `LAUNCH_FAILED` and an unknown code. A
refusal of the consent (`DENIED`, `UNAVAILABLE`, `BUSY`, `RATE_LIMITED`), `NO_PROJECT` and `NO_DEFAULT` MUST say what
happened and what the user can do.

#### Scenario: The words of every refusal code
- **WHEN** the IDE refuses the launch
- **THEN** the dialog words every refusal code of the IDE
- **AND** it shows the message of the IDE only for `LAUNCH_FAILED` and an unknown code
  [@test] src/lib/air-launch.test.ts (`words every refusal code of the IDE, the message of a failed launch, and an unknown code with its message`)

### Requirement: The session toast: The toast follows the session in place

With a local session id, the toast "Started Codex in Air" MUST follow the row of the session in place: "Codex works in
Air…" (`processing`, an activity of a later IDE, and none), "Codex waits for you in Air" (`needs_input`, `draft`),
"Codex waits for your review in Air" (`reviewing`), with the title of the session.

#### Scenario: The words of each activity
- **WHEN** the session reports an activity
- **THEN** `processing`, an activity of a later IDE and none read "Codex works in Air…"
- **AND** `needs_input` and `draft` read "Codex waits for you in Air", `reviewing` reads "Codex waits for your review
  in Air"
- **AND** the row names its activity in any case, with or without `_` (`needsInput`, `NEEDS_INPUT`)
  [@test] src/lib/air-launch.test.ts (`words the activity as working, waiting for the user or done, and a later activity as working`)
  [@test] crates/air-launch/src/protocol_test.rs (`the_activity_of_a_row_is_read_in_any_case_with_or_without_underscores`)

#### Scenario: The toast follows each status in place
- **WHEN** a launch is accepted with a local session id
- **THEN** the toast "Started Codex in Air" watches the session and shows each status in place, with the title of the
  session
  [@test] src/lib/air-session-toast.test.ts (`says it started, follows each status in place, and stops the watch once the session is done`)

### Requirement: The session toast: The toast says how the watch ended

A removed row MUST read "The session of Codex is gone in Air"; a close, no IDE, a refusal and an error MUST say that the
toast stopped following, and why.

#### Scenario: The session is gone
- **WHEN** the watch ends because the row of the session is removed
- **THEN** the toast says "The session of Codex is gone in Air"
  [@test] src/lib/air-session-toast.test.ts (`says the session is gone, and a close of the toast stops the watch`)

#### Scenario: The end of a watch
- **WHEN** the watch ends with a close, a refusal or an error
- **THEN** the toast says that it stopped following, and why
- **AND** it says nothing for a watch the page stopped
  [@test] src/lib/air-launch.test.ts (`words the end of a watch, and nothing for a watch the page stopped`)

### Requirement: The session toast: One watch per window and session

The app MUST hold one watch per window and session (`air_watch_session`), and one connection per IDE for all of them: a
newer watch of the session stops the one before, `stop_air_watch` stops it, the page of its window that goes away stops
its watches, and the last watch of an IDE that ends closes its connection.

#### Scenario: A stop, a newer watch and a page that went away
- **WHEN** `stop_air_watch` names the session, a newer watch of the session starts, or the page of the window goes away
- **THEN** the watch before ends
- **AND** a newer watch keeps the connection of the IDE, and the stop of the last watch closes it
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_stop_a_newer_watch_of_the_session_and_a_page_that_went_away_end_the_watches_of_a_window`)

#### Scenario: A page that went away
- **WHEN** the page of the window went away (a reload, a close)
- **THEN** its watch stops at its first status, though its Tauri channel never fails its send
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_stop_a_newer_watch_of_the_session_and_a_page_that_went_away_end_the_watches_of_a_window`)

#### Scenario: The page watches a session and stops it
- **WHEN** the page watches a session through `air_watch_session`
- **THEN** it gets each status, then the end
- **AND** its stop names the local session id to `stop_air_watch`
  [@test] src/lib/air-launch.test.ts (`watches a session: each status to the callback, then the end; the stop names the session`)

### Requirement: The session toast: A watch tells the page each status and how it ended

A watch MUST tell the page the status of the row whose `resource` is its local session id, now and after each change
of its title or activity, and how it ended: `noIde`, `refused` with its code (the connection or the list), `gone` (the
row is removed), `closed`, or `stopped`. A watch whose row is not there yet MUST wait for it.

#### Scenario: Each status, then the end
- **WHEN** a watch runs to its end
- **THEN** the page gets each status, then `refused` with its code, `gone`, `closed`, or `stopped`
- **AND** a change of another row, or of another field of the row, tells the page nothing
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_watch_tells_the_page_the_status_of_its_row_now_and_after_each_change_then_that_it_is_gone`, `a_refusal_of_the_list_ends_each_watch_refused`)

#### Scenario: A close the page stopped
- **WHEN** the IDE closes the connection
- **THEN** each watch of the IDE ends `closed`
- **AND** the watches of one IDE share its connection, and a watch of a row that comes later follows it
  [@test] crates/app-api/src/app/air_launch_test.rs (`the_watches_of_one_ide_share_one_connection_and_a_row_that_comes_later_is_followed`)

## REMOVED Requirements

### Requirement: The exchange: A refusal of v1 can come before the first status

**Reason**: The IDE has no watch limit; a refusal of the connection or of the list ends the watch.
**Migration**: "The session toast: A watch tells the page each status and how it ended".

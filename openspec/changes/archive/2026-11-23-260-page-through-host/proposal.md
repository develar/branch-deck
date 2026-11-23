# Proposal

## Why

The host of the one backend landed (`crates/backend-host`), but the pages of the desktop app still called app-api
through the typed Tauri wrappers, beside the host: the app's windows and the IDE's had one session per repository
only by the shared sessions of the process, and the page's OS notification of a news claimed nothing, so the app and
the IDE could both tell a failure (#260, plan item 2.6;
[Backend Protocol](../../../docs/design/backend-protocol.md), "The pages of the app").

## What Changes

- Every `invoke` of a page of the app goes through the in-process connection of the host (`backend_host::page`): a
  command is the root command of its window, a subscription the subscription of its window on the repository channel,
  each answer and patch the text of its JSON; the page keeps its bindings, `src/model/store.ts` and
  `src/model/intents.ts`.
- The page mock of the e2e server answers the same way, through one host per test repository, which the connections
  of the IDE attach to.
- The page claims a news before its OS notification (the command `claim_news`), and the claim names what the news
  tells (`NewsClaim`), never its id, which each session counts from 1.
- A command that panics fails its call (`-32603 panic`) instead of leaving it without an answer; a subscribe of a
  window whose earlier subscribe still joins is a reservation of its own.

Capabilities: `repository/backend-protocol`.

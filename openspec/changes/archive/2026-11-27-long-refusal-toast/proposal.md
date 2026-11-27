# Proposal

## Why

A push that a mirror refused (Space forwarding to GitHub, `index-pack failed`) showed an error toast taller than the
window: every refused ref repeated the same words, and the lines of the remote behind the mirror (`remote: remote: …`),
its progress among them, were joined into the description. The title and the close button were above the window, so
the toast could not be closed.

## What Changes

- Every toast shows at most 4 lines of its description, with "Show More", which shows the rest in the toast, scrolled
  within it, and "Show Less" (`ClampedToastDescription` of `src/components/ui/toast.tsx`).
- A refused push names the refs of the same report together, once with its words; the lines of the remote keep one
  `remote:`, the progress of any stage is left out, a line said again is said once (`remote_messages`, `send_leased`).

Capabilities: `branch/landing-routes`, `cli/agent-cli`.

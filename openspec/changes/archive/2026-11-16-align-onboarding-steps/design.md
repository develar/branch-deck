## Context

The repository button currently owns padding that the informational rows lack. See proposal.md for the motivation. Onboarding tracking and prefix focus already live outside presentation.

## Decisions

Use one row geometry for the button and informational rows. A ghost button removes the visible outline while retaining shared interaction and focus feedback. Keep the numbered badge in every state and put completion beside the title, so neither column moves. Neutral badges and muted upcoming text express dependency without adding controls.

Keep the current prefix form, help and lifecycle untouched. Update the existing onboarding contract and UI theme documentation alongside focused browser coverage.

Color consistently describes progress: active numbers and titles use primary, upcoming steps are muted, and completed numbers and titles stay neutral with a success check. Apply this to prefix loading, standalone setup and hosts without a picker too; document the rule beside the shared step renderer.

## Risks / Trade-offs

- Shared button defaults can add a transparent border or icon padding → explicitly remove the border and verify actual column positions in Chromium and WebKit.
- Muting can reduce legibility → use semantic muted foreground tokens and inspect both themes.

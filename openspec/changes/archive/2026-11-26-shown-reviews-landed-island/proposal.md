# Proposal

## Why

A branch that landed and left the table (its robot or its pull request landed it, the update took it out) could get its
review after landing only by `brd request-review --after-landing`: the Landed island offered nothing.

## What Changes

- The actions of the view hold `landed:<name>` of each branch of the island that left the table
  (`domain_policy::actions::landed_actions`): Request Review After Landing…, refused without Patronus or GitHub, with its
  review after landing there already, and while nothing tells the commit that landed it (`Reason::ShowNothingLanded`).
- Its row of the island offers it as a button, whose form under the row picks the reviewers (the code owners of what
  landed first) and opens the review ready on what landed (`Intent::MarkShownReview`).

Capabilities: `repository/landed-commits`, `branch/shown-reviews`.

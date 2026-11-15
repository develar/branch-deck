# The review window fetches the request as the list had it last

## Why

The review of 0b4275122 found two gaps. The fetch of a stack (Review Stack) had no test of the rule of a failure that
stays until Retry, and the pushed request did not check that the failure before it went. And older: the window took
the request it shows from the list, else from the request it opened on (`live ?? shown.request`), so a request pushed
while its review was open and then withdrawn (approved, merged) fell back to the head of the open, another key: the
window fetched it again, the ranges of the review were read anew, and a failure of that needless fetch showed on a
review that was fine.

## What Changes

- The window keeps the last request the list had (`shown.request` follows `live`), so a withdrawn request is the pushed
  one it fetched.
- The store tests of the fetch of a stack (a failure stays on a second start of the same key, Retry fetches again, a
  pushed or rebased layer is another key), and of the failure of the request before a push that goes.
- Capability `repository/review-requests`.

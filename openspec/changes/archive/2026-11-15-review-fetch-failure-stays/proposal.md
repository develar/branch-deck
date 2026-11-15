# A failed fetch of a review stays until Retry

## Why

The review window asks for the fetch of the request it shows in an effect of the request object, and a failed fetch
was fetched again on any start. The session gives the window the same request anew (a patch of the review requests),
so the window fetched a failure again of itself: the callout went while the user moved to its Retry, and the e2e test
of Retry waited for a button that was gone (`review-requests.spec.ts`, "should say why a fetch failed and fetch again
on Retry", 2 of 20 and 1 of 25 runs; a probe showed the start from the effect during the click).

## What Changes

- `startReviewFetch` and `startReviewStackFetch` start nothing for the latest fetch of the window when it stands:
  fetched, being fetched, or failed; `retryReviewFetch` and `retryReviewStackFetch`, the Retry of the callout, fetch a
  failed one again. Another request, a pushed one too, is fetched at once.
- Capability `repository/review-requests`.

import { BRANCH_REVIEW_WINDOW, COMMIT_REVIEW_WINDOW, windowLabel } from "@/lib/window-label"

// A review window reads its subject while the chunk of its page loads, which the reads need nothing of; no other window
// loads them. `src/main.tsx` imports this module before the router, whose start asks for the chunk of the page, so the
// small chunk of the reads is asked first and the reads start as the page does (`startReviewWindowReads`)
if (windowLabel === COMMIT_REVIEW_WINDOW || windowLabel === BRANCH_REVIEW_WINDOW) {
  void import("@/lib/review-window-reads").then(reads => reads.startReviewWindowReads(windowLabel))
}

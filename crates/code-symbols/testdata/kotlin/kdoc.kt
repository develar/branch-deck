package com.example.doc

/**
 * Starts a [Session] for the given TurnTracker.
 * @param tracker the TurnTracker to use
 */
fun start(tracker: Tracker) {
  // TurnTracker is legacy
  /* TurnTracker again */
  println("TurnTracker: $tracker ${tracker.turnTracker}")
  println("version $VERSION of ${Build.number()}")
}

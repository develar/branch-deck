# Tasks

- [x] `ReviewRequestsFact`: `heads` and `stacks` apart from `kept`; `fetched` and `stack_fetched` keep the head
  whatever the windows did, the refs only of a fetch that is still kept; `fetch_ended` drops one that ended without a
  head while no window shows it and no other fetch of it runs (`in_flight`); a closed stack's layers stay while another
  stack that is kept has them; the view shows the stacks whose top a window shows
- [x] `Message::ReviewFetchEnded` and `Sessions::review_fetch_ended`, sent by the drop of `CountedFetch` in
  `operations::review_fetch`; its end prunes only when the set the prunes keep changed
- [x] `ReviewPrunes`: one task runs the prunes in order, the newest waiting one in place of an older one;
  `ReviewFetching` answered once the prunes decided before it ended; `CountedFetch::start` ends that wait at a stop of
  the operation (`StoppedCommand::Cancelled`) or after 120 s
- [x] `told_kept_reviews` is `None` until the first prune, which prunes whatever it keeps; `tell_demand` decides a prune
  only when the review windows changed; a stack goes once no window shows its top; the limit of the wait of a fetch is a
  parameter (`fetch_review_within`)
- [x] The redundant `review_requests.demand` of `send_patch` goes: `tell_demand` is the only writer of `told_demand`
- [x] Tests: the fact (`a_fetch_that_ends_after_its_window_moved_on_keeps_nothing`,
  `a_fetch_keeps_its_head_for_its_window_when_another_window_dropped_its_pull_request_while_it_ran`,
  `the_layers_of_a_closed_stack_stay_while_another_stack_has_them`, `a_failed_fetch_of_a_layer_leaves_the_stack_that_has_it_whole`,
  `a_fetch_that_ends_without_a_head_goes_unless_a_window_shows_it`,
  `a_fetch_before_the_demand_of_its_window_keeps_its_head_until_the_window_closes`,
  `a_stack_window_opened_again_shows_no_stack_until_its_own_fetch_ended`), the session
  (`the_first_prune_of_a_session_that_keeps_nothing_prunes_once`,
  `a_fetch_that_ends_after_its_window_moved_on_keeps_none_of_its_refs`,
  `a_fetch_that_ends_without_a_head_keeps_its_refs_only_while_its_window_shows_them`,
  `a_fetch_keeps_its_head_when_another_window_drops_its_pull_request_while_it_runs`,
  `the_prunes_of_a_session_run_in_the_order_it_decided_them`, `the_prunes_decided_while_one_runs_run_as_the_newest`,
  `a_review_fetch_writes_once_the_prunes_decided_before_it_ended`), the operation
  (`a_review_fetch_that_fails_keeps_nothing_for_the_prunes`, `a_stop_or_a_drop_of_a_review_fetch_that_waits_for_a_prune_ends_it`,
  `a_review_fetch_that_waits_for_a_prune_past_its_limit_ends`)
- [x] The e2e hold of the late demand is `holdCommandCalls` with `when`

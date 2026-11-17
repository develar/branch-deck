# Tasks

- [x] `Message::ReviewFetching` and `ReviewRequestsFact::fetching`: a fetch of a pull request (of a stack its top) counts
  from its start; `prune_kept_reviews` keeps `fetched_numbers`; `tell_demand` gives the review requests the demand
  before the prune; the close of a stack's window drops its layers but those another window shows
- [x] `a_fetch_before_the_demand_of_its_window_keeps_its_refs_at_a_listing_that_withdraws_it` (session) and
  `the_fetches_of_a_window_keep_their_numbers_until_it_closes_with_the_layers_of_its_stack` (the fact)
- [x] The e2e test keeps its assertion: one prune, `[101, 102]`; a new e2e test holds the demand of the window that
  names #101 until the listing withdrew it, and the prunes are `[101, 102]`, then `[101]` once #102 goes too

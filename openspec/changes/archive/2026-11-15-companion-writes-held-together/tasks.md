# Tasks

- [x] The write under the companion locks of both repositories, asked again under them; the app's writers of both
  configs held in the order of their files; the cross-process tests (another process holds a lock while it adds,
  removes, or claims the companion for another primary), the moved companion, with mutations (no locks, the locks after
  the reads, no second ask)
- [x] The order of the review through the app's writers (an add and a removal of one linked pair at once, in both
  orders), which fails on the three-step write; a write together holds its writers until it ends; two of one pair named
  in opposite orders both end (taken unsorted they wait for each other forever)
- [x] Half links in Settings, `brd companion list` and their repair, the Rust test and the e2e of both sides, with
  mutations (no half link, no problem of the primary, a gone primary that lists the companion)
- [x] Git Operations (the companion registry, "Writes at once"), Git Rules, `brd companion --help`, the Code Map

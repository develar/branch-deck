## 1. The rows

- [x] 1.1 `StepKind::AfterLanding` of `next_step::decide`, its node and Status cell; `NeedsBranch` with its edges;
  `brd status` `after_landing`
- [x] 1.2 The presenters word the edges (`describeEdge`, `formatLandsAfter`, `formatEdgeFix`); the actions of a fix
  of an edge (the restart of the robot of the needed branch, the group, the start)

## 2. The popover and the toast

- [x] 2.1 The headline and the detail of a refusal, its fix focused, Start Anyway `outline` or in "⋯" of a wait, the
  lines wrapped
- [x] 2.2 The start that waited for a merge, kept from the view (`noteWaitingStarts`) and offered by the toast of the
  update (`takeStartAfterMerge`)
- [x] 2.3 The template `landed_need` of the E2E server and `tests/e2e/next-step/lands-after.spec.ts`

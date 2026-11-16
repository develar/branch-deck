# A plan decided once admitted runs on what its operation holds

## Why

Issue #237, "The same pattern in four more operations" (the body of 190ec858): a start, a push, a pull request intent
and a close were admitted with what their plan as the session had it held (nothing where it had none), then decided
again once admitted, and ran that plan whatever it needed. A plan that needed more (a branch a hold no longer kept, the
ref of an ancestor whose remote branch went, a subject the table had not yet, a pull request listed since) ran beside
the operations that held it. The restart had a check of its own that refused such a plan.

## What Changes

- One check in the engine (`engine::Holding::cover`), before the first effect of a start (both of its plans), a push, a
  pull request intent, a restart and a close: what the plan decided once admitted holds, by the builder of its
  admission, is held (HEAD aside); the first time it needs more, the operation holds that (`RunningGuard::hold`) after
  the operations of this process that hold it, and is decided again on what they left; a second time it is refused
  `plan_changed` before its first effect. The restart moves onto it from its own refusal.
- It holds through `RunningGuard::hold`, which holds for every process and never waits for an operation that waits for
  it (the change before, `an-operation-that-holds-more-holds-it-for-every-process`).
- The admissions hold the remote effects of their plan alike (`engine::remote_holds`): the base of each pull request a
  retarget moves or a link brings in line, the robot a cancel cancels, the robot a restart restarts with what it lands.
  A push holds besides the base of the open pull request of each branch of its stack and, restarting the Safe Push of
  its branch, every robot listed of the branch.
- Without a plan as the session has it (the frame of an earlier run, a subject the table lacks) an admission holds what
  the intent names of its subject: the branch a start lands, the head a pull request intent pushes, the pull request a
  close closes; what the plan once admitted needs more the cover holds.
- A push decided again after a wait of its hold asks the robots again. A start is planned over the host that needs no
  Patronus and runs over the host of the route its plan decided once admitted.
- Capability `branch/patronus`: the restart's requirement changes, two requirements of the operations are added.

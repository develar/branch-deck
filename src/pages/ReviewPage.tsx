import { Suspense, lazy, useEffect, useEffectEvent, useRef, useState } from "react"
import type { ReactNode } from "react"
import { useStore } from "zustand"
import { Card } from "@/components/ui/card"
import { BranchReviewActions, BranchReviewBar, BranchReviewBody, BranchReviewGone, BranchReviewMarkReviewed } from "@/components/review/BranchReview"
import type { BranchReviewMoves } from "@/components/review/BranchReview"
import { RangeReview } from "@/components/review/RangeReview"
import { ReviewBackButton } from "@/components/review/ReviewBackButton"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { ReviewRobotStatus } from "@/components/review/ReviewRobotStatus"
import { ReviewSidebarToggle } from "@/components/review/ReviewSidebarToggle"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { WindowTitle } from "@/components/shared/WindowTitleBar"
import { hasCommitKeys, keyedAction, useBranchReview } from "@/hooks/useBranchReview"
import type { BranchReviewModel } from "@/hooks/useBranchReview"
import { useLinkedFeedback } from "@/hooks/useMainWindowLink"
import { useReviewActions } from "@/hooks/useReviewActions"
import type { ReviewActionItem } from "@/hooks/useReviewActions"
import { useSubjectPlan } from "@/hooks/useReviewPlan"
import { useReviewPosition } from "@/hooks/useReviewPosition"
import { useSessionFactStores } from "@/hooks/useSessionFactStores"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { useWindowTitle } from "@/hooks/useWindowTitle"
import { branchGoneReason, keptGoneReason, reviewCommitKey, sameReviewTable, settledKeptTable, settledReviewTable } from "@/lib/branch-review"
import type { ReviewedTable } from "@/lib/branch-review"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import type { MenuItemGroups } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { isMac } from "@/lib/platform"
import { openPullRequestUrl } from "@/lib/pull-request"
import { showReviewToast } from "@/lib/pull-request-review"
import { commitItems, isSingleCommit } from "@/lib/range-review"
import { planFacts } from "@/lib/review-plan-answer"
import { pushReviewHistory, restoredReview, reviewSubjectName, reviewWindowTitle, takeOpen } from "@/lib/review-reuse"
import type { ReviewPlace, ShownReviewWindow } from "@/lib/review-reuse"
import { askReviewRun } from "@/lib/review-runs"
import type { ReviewRunFeedback } from "@/lib/review-runs"
import { openBranchReviewWindow, openWholeStackReviewWindow, reviewBranchCommit } from "@/lib/review-window"
import { showToast } from "@/lib/toast-message"
import { formatWholeStack, stackReadinessKey } from "@/lib/whole-stack"
import { REVIEW_WINDOW } from "@/lib/window-label"
import { showReviewReadiness } from "@/model/demand"
import { ensureSession, viewVersionKey } from "@/model/store"
import { archivedBranchesStore } from "@/stores/archived-branches"
import { branchSyncStore, getBranch, isForkingRoot, mainLineOf, whenTableSettled } from "@/stores/branch-sync"
import { useCreatePullRequestBranch } from "@/stores/create-pull-request-dialog"
import { unappliedBranchesStore } from "@/stores/unapplied-branches"
import type { KeptReview, ReviewWindowData } from "@/types/sub-window"

// loaded on the first open, then kept mounted (the dialog animates and returns the focus when it closes)
const CreatePullRequestDialog = lazy(() => import("@/components/branch/CreatePullRequestDialog").then(it => ({ default: it.CreatePullRequestDialog })))

/** Where a review of the window says it is, by the start of the review: Back restores it there */
type ReportPlace = (place: ReviewPlace) => void

type ShownBranchReview = Extract<ShownReviewWindow, { kind: "branch" }>
type ShownCommitsReview = Extract<ShownReviewWindow, { kind: "commits" }>

/**
 * The review window (openspec/specs/review/review-window/spec.md#one-window), one for every subject: a branch with its stack, the
 * whole stack of a root, a kept branch (`BranchReview` on the canvas of the window), or a list of commits (the commits
 * above the files, `RangeReview`). It gets every open and decides how it goes on from the review on screen
 * (`takeOpen`, the one rule `planReviewOpen`): an open of the review on screen keeps it (a move shows its commit); a new
 * start shows the new subject and keeps the one it left in the history of the window, at the place it was: Back (⌘[,
 * the button at the start of the title bar) starts that one over. A start over while the "Create pull request" dialog
 * holds what the user typed is refused. The window holds the session of its repository and the stores of its facts,
 * from which it builds the actions of what it shows (`useReviewActions`).
 */
export function ReviewPage() {
  const incoming = useSubWindowData<ReviewWindowData>()
  const dialogBranch = useCreatePullRequestBranch()
  // the open last taken, the review on screen, the reviews it left (the latest last), an open the dialog refused
  const [taken, setTaken] = useState<ReviewWindowData | null>(null)
  const [shown, setShown] = useState<ShownReviewWindow | null>(null)
  const [history, setHistory] = useState<readonly ShownReviewWindow[]>([])
  const [refused, setRefused] = useState<{ subject: string; dialog: string; at: number } | null>(null)
  if (incoming !== taken) {
    setTaken(incoming)
    if (incoming !== null) {
      // the open names its own start: the same open always takes the same way
      const next = takeOpen(shown, incoming, incoming.openedAt)
      if (next.restarted && dialogBranch !== null) {
        setRefused({ subject: reviewSubjectName(incoming), dialog: dialogBranch, at: incoming.openedAt })
      } else {
        if (next.restarted && shown !== null) {
          setHistory(pushReviewHistory(history, shown))
        }
        setShown(next.shown)
      }
    }
  }
  useEffect(() => {
    if (refused !== null) {
      showToast({
        type: "info",
        title: `Cannot review ${refused.subject}`,
        description: `The review window has the pull request of ${refused.dialog} open: create it or close the dialog first`,
      })
    }
  }, [refused])
  // where each review of the window is, by its start
  const places = useRef(new Map<number, ReviewPlace>())
  const repositoryPath = shown?.repositoryPath ?? null
  // the session of the window's repository, under the window's label: its patches reach this window as the main
  // window's reach it, also while the main window shows another repository; it ends with the window
  useEffect(() => {
    if (repositoryPath !== null) {
      ensureSession(repositoryPath)
    }
  }, [repositoryPath])
  // the stores of its facts (the robots, the pull requests, the readiness, the review requests), as the main window's
  // are: the actions read them; the news of the session are the main window's
  useSessionFactStores()
  // why the main window refused an action the window asked it to run at its row, the toast of a Safe Push it started
  useLinkedFeedback<ReviewRunFeedback>(REVIEW_WINDOW, showRunFeedback)

  const back = () => {
    const previous = history.at(-1)
    if (previous === undefined) {
      return
    }
    const restored = restoredReview(previous, places.current.get(previous.startedAt), Date.now())
    const rest = history.slice(0, -1)
    // only the reviews still in the history keep their place
    for (const startedAt of places.current.keys()) {
      if (!rest.some(it => it.startedAt === startedAt)) {
        places.current.delete(startedAt)
      }
    }
    setHistory(rest)
    setShown(restored)
  }
  const handleBackKey = useEffectEvent((event: KeyboardEvent) => {
    // ⌘[ on macOS, Ctrl+[ elsewhere, as Back of a browser; not in a dialog of the window
    const modifier = isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey
    if (event.code !== "BracketLeft" || !modifier || event.altKey || event.shiftKey || event.defaultPrevented) {
      return
    }
    if (event.target instanceof Element && event.target.closest("[role=dialog]") !== null) {
      return
    }
    event.preventDefault()
    back()
  })
  useEffect(() => {
    const listener = (event: KeyboardEvent) => handleBackKey(event)
    window.addEventListener("keydown", listener)
    return () => window.removeEventListener("keydown", listener)
  }, [])

  const [isDialogLoaded, setDialogLoaded] = useState(false)
  if (dialogBranch !== null && !isDialogLoaded) {
    setDialogLoaded(true)
  }

  if (shown === null) {
    return null
  }
  const previous = history.at(-1)
  const backButton = previous === undefined ? null : <ReviewBackButton name={reviewSubjectName(previous)} onClick={back} />
  const startedAt = shown.startedAt
  const reportPlace: ReportPlace = place => {
    places.current.set(startedAt, place)
  }
  const key = `${shown.repositoryPath}\n${startedAt}`
  const shownRepository = shown.repositoryPath
  return (
    <>
      {shown.kind === "commits" ? (
        <CommitsReviewWindow key={key} data={shown} back={backButton} onPlace={reportPlace} />
      ) : shown.kind === "pullRequest" || shown.kind === "pullRequestStack" ? (
        <LazyPullRequestReview key={key} data={shown} back={backButton} onPlace={reportPlace} />
      ) : (
        <BranchReviewWindow key={key} data={shown} back={backButton} onPlace={reportPlace} dialogOpen={dialogBranch !== null} />
      )}
      {/* outside the review: its keys never see what is typed into the dialog */}
      {isDialogLoaded && (
        <Suspense fallback={null}>
          <CreatePullRequestDialog
            safePushWithoutReview={branchName => void askReviewRun({ repositoryPath: shownRepository, subject: branchName, itemId: "safe-push-without-review" })}
          />
        </Suspense>
      )}
    </>
  )
}

/** The module of the review of a pull request, a chunk of its own; null until it loaded */
let pullRequestModule: typeof import("@/pages/PullRequestReviewWindow") | null = null

/** Loads the chunk of the review of a pull request (asked already with the page, `startReviewWindowReads`) */
function loadPullRequestReview(): Promise<typeof import("@/pages/PullRequestReviewWindow")> {
  return import("@/pages/PullRequestReviewWindow")
}

/**
 * The review of a pull request or of a stack of them once its chunk loaded (`src/lib/review-window-reads.ts` asks for
 * it with the page): no `React.lazy` under `Suspense`, whose fallback would hold the content back (`src/router.ts`); the
 * window shows by its content anyway (`readyByContent`)
 */
function LazyPullRequestReview(props: { data: Extract<ShownReviewWindow, { kind: "pullRequest" | "pullRequestStack" }>; back: ReactNode; onPlace: ReportPlace }) {
  const [loaded, setLoaded] = useState(pullRequestModule)
  useEffect(() => {
    if (loaded !== null) {
      return
    }
    let current = true
    loadPullRequestReview().then(
      module => {
        pullRequestModule = module
        if (current) {
          setLoaded(module)
        }
      },
      (error: unknown) => notifyError("Cannot load the review of the pull request", error),
    )
    return () => {
      current = false
    }
  }, [loaded])
  if (loaded === null) {
    return null
  }
  const { data, back, onPlace } = props
  return data.kind === "pullRequestStack" ? (
    <loaded.PullRequestStackReviewWindow data={data} back={back} />
  ) : (
    <loaded.PullRequestReviewWindow data={data} back={back} onPlace={onPlace} />
  )
}

/** A refusal of the main window, or the toast of the Safe Push of a pull request it started for the window */
function showRunFeedback(feedback: ReviewRunFeedback): void {
  if (feedback.kind === "error") {
    notifyError(feedback.title, feedback.message)
    return
  }
  showReviewToast(feedback.toast)
}

/**
 * What the review reads of the table of the window's session (`settledReviewTable`): taken when the view settles, so
 * once per finished sync, whichever window asked for it, and never from a run that is rebuilding the copies; null
 * until a sync grouped the branches
 */
function useReviewedTable(chain: readonly string[], wholeStack: boolean, kept: KeptReview | undefined): ReviewedTable | null {
  const chainKey = chain.join("\n")
  const keptKind = kept?.kind
  const keptName = kept?.name
  const [table, setTable] = useState(() => settledTable(chain, wholeStack, kept))
  useEffect(() => {
    const names = chainKey.split("\n")
    const of = keptKind === undefined || keptName === undefined ? undefined : { kind: keptKind, name: keptName }
    const take = () => {
      const next = settledTable(names, wholeStack, of)
      if (next !== null) {
        setTable(previous => (sameReviewTable(previous, next) ? previous : next))
      }
    }
    take()
    // a kept branch is in the stores of its section: the unapply, the reapply and the delete of the session patch them
    const unsubscribe = [branchSyncStore.subscribe(take), unappliedBranchesStore.subscribe(take), archivedBranchesStore.subscribe(take)]
    return () => unsubscribe.forEach(it => it())
  }, [chainKey, wholeStack, keptKind, keptName])
  return table
}

/** The table of the window's session as the review of `chain` reads it now (`settledReviewTable`, `settledKeptTable`) */
function settledTable(chain: readonly string[], wholeStack: boolean, kept: KeptReview | undefined): ReviewedTable | null {
  const state = branchSyncStore.getState()
  return kept === undefined
    ? settledReviewTable(state, chain, wholeStack)
    : settledKeptTable(state, kept, chain[0] ?? "", unappliedBranchesStore.getState(), archivedBranchesStore.getState())
}

/**
 * The review of a kept branch that was reapplied goes on as the review of the branch it is again, once a sync built it:
 * the window opens it itself, as the menu of the row would
 */
function useReappliedReview(data: ShownBranchReview, gone: boolean): void {
  const kept = data.kept
  const branchName = data.chain[0] ?? ""
  const rebuilt = useStore(branchSyncStore, state => kept !== undefined && gone && state.branches[branchName] !== undefined)
  const [asked, setAsked] = useState(false)
  if (rebuilt && !asked) {
    setAsked(true)
  }
  useEffect(() => {
    if (asked) {
      void openBranchReviewWindow(branchName)
    }
  }, [asked, branchName])
}

/**
 * The readiness of the branches the review shows the Safe Push of, while it shows them: their next step reads it, as a
 * row's does (of a whole stack the next step of the whole stack, as the header of its island does); none of a kept branch
 */
function useShownReadiness(chain: readonly string[], wholeStack: boolean, kept: boolean): void {
  const subjects = kept ? "" : wholeStack ? stackReadinessKey(chain[0] ?? "") : chain.join("\n")
  useEffect(() => {
    if (subjects === "") {
      return
    }
    const hides = subjects.split("\n").map(subject => showReviewReadiness(subject))
    return () => hides.forEach(hide => hide())
  }, [subjects])
}

/** The place of a review of a branch: its part by the names of its ends, the filter, the commit, the file */
function branchPlace(model: BranchReviewModel): ReviewPlace | null {
  const { loaded } = model
  if (loaded === null) {
    return null
  }
  const from = loaded.segments[loaded.range.from]?.name
  const to = loaded.segments[loaded.range.to]?.name
  return {
    kind: "branch",
    focus: {
      range: from === undefined || to === undefined ? "own" : { from, to },
      filter: model.filter,
      commitId: loaded.subject.kind === "commit" ? loaded.subject.id : null,
      path: model.lastPath,
    },
  }
}

/**
 * The review of a branch (`BranchReview` on the canvas of the window): the stack bar, Inline/Unified/Split and the actions on
 * the branch of the part in the title bar, the commits and the owner chips above the files beside the diffs in the
 * island, the position and the keys in the footer. The heads a sync built, the baseline, the branch of each commit of a
 * whole stack and the actions come from the window's own session; the window runs the actions itself, those that open
 * a form under the row of their subject by the main window (`review/run`).
 */
function BranchReviewWindow({ data, back, onPlace, dialogOpen }: { data: ShownBranchReview; back: ReactNode; onPlace: ReportPlace; dialogOpen: boolean }) {
  // the window's view of the session: the heads the last sync built, the baseline
  const wholeStack = data.wholeStack === true
  const table = useReviewedTable(data.chain, wholeStack, data.kept)
  const viewVersion = useStore(branchSyncStore, state => viewVersionKey(state.viewVersion))
  // the review plan, as the last sync of the window's session read `branchdeck.reviewExperimental`
  const reviewExperimental = useStore(branchSyncStore, state => state.reviewExperimental)
  const baselineBranch = table?.baselineBranch ?? data.baselineBranch
  // what the window can do with the branches, from its own session
  const actions = useReviewActions({ repositoryPath: data.repositoryPath, baselineBranch, chain: data.chain, wholeStack, kept: data.kept })
  useShownReadiness(data.chain, wholeStack, data.kept !== undefined)
  // the reviewer groups of the check that answered while the window is open, else those of the open; a move to a commit
  // waits while the "Create pull request" dialog is open
  const model = useBranchReview(
    { ...data, baselineBranch, groups: actions.groups === null ? data.groups : [...actions.groups], reviewExperimental },
    data.focus,
    { known: table !== null, tips: table?.tips ?? {} },
    { openedAt: data.openedAt, focus: data.move ? data.focus : null, held: dialogOpen },
  )
  // the actions of each branch as the session had it last: they stay, disabled, for a branch that left
  const [seenItems, setSeenItems] = useState(actions.items)
  const [knownItems, setKnownItems] = useState(actions.items)
  if (actions.items !== seenItems) {
    setSeenItems(actions.items)
    setKnownItems({ ...knownItems, ...actions.items })
  }

  const { loaded } = model
  useReappliedReview(data, model.gone.length > 0)
  useWindowTitle(reviewWindowTitle(data))
  const place = branchPlace(model)
  useEffect(() => {
    if (place !== null) {
      onPlace(place)
    }
  })
  const target = loaded?.target ?? data.chain.at(-1) ?? ""
  const gone = model.gone.includes(target)
  // up to the whole stack and down to a branch of it: the window opens the review as the menus of the rows would
  const moves: BranchReviewMoves = {
    wholeStack: actions.wholeStack,
    onWholeStack: () => void reviewWholeStackOf(data.chain, actions),
    from: data.from ?? [],
    stackCommits: table?.stackCommits ?? noStackCommits,
    commitMenus: actions.commitMenus,
    onBranch: (branchName, commitKey) => void reviewBranchOf(branchName, commitKey),
  }
  const items = loaded === null ? [] : shownItems(knownItems[loaded.target] ?? [], gone ? goneReason(loaded.target, data.kept) : null)
  const run = useBranchReviewActions(items, dialogOpen)
  return (
    <WindowPage
      data-testid="review-window"
      data-view-version={viewVersion ?? undefined}
      body="fill"
      readyByContent
      title={
        <>
          {back}
          <BranchReviewBar model={model} moves={moves} />
          {/* the robot that lands what the review shows, while it runs */}
          {loaded !== null && data.kept === undefined && <ReviewRobotStatus target={loaded.target} wholeStack={wholeStack} />}
        </>
      }
      titleEnd={
        loaded !== null && (
          <>
            <ReviewSidebarToggle />
            <DiffViewToggle />
            <div className="flex items-center gap-1" data-testid="branch-review-actions">
              <BranchReviewMarkReviewed model={model} />
              <BranchReviewActions actions={items} branchName={loaded.target} onRun={run} />
              <BranchReviewMenu
                name={wholeStack ? formatWholeStack(loaded.target) : loaded.target}
                groups={actions.menus[loaded.target] ?? []}
                goneReason={gone ? goneReason(loaded.target, data.kept) : null}
              />
            </div>
          </>
        )
      }
      footer={
        loaded !== null && (
          <PreviewDialogFooter
            className="border-t-0 px-1 py-0"
            data-testid="review-window-footer"
            start={
              <span className="tabular-nums" data-testid="review-position">
                {loaded.position}
              </span>
            }
          >
            <ReviewKeyHints commits={hasCommitKeys(model)} escape="Close" />
          </PreviewDialogFooter>
        )
      }
    >
      <BranchReviewGone model={model} />
      <Card className="min-h-0 flex-1 flex-row gap-0 overflow-hidden py-0">
        <div className="min-w-0 flex-1" aria-busy={model.reading || undefined} data-testid="branch-review-body">
          <BranchReviewBody model={model} moves={moves} />
        </div>
      </Card>
    </WindowPage>
  )
}

const noStackCommits: Readonly<Record<string, string>> = {}

/**
 * "Whole Stack" of the stack bar of `chain`: the review of the whole stack of its root, the chain its way back; of a
 * straight chain the review of its last branch from the root, all of it
 */
function reviewWholeStackOf(
  chain: readonly string[],
  actions: { readonly wholeStack: { readonly root: string; readonly disabled?: boolean; readonly description: string } | null },
): Promise<boolean> {
  const scope = actions.wholeStack
  const root = chain[0] ?? ""
  if (scope === null || scope.disabled === true) {
    notifyError(`Cannot review ${formatWholeStack(root)}`, scope?.description ?? `${root} heads no stack beyond ${chain.at(-1)}`)
    return Promise.resolve(false)
  }
  if (isForkingRoot(branchSyncStore.getState(), root)) {
    return openWholeStackReviewWindow(root, { from: chain })
  }
  return openBranchReviewWindow(mainLineOf(branchSyncStore.getState(), root).at(-1) ?? root, { range: "all" })
}

/**
 * The review of the branch `name` of the stack on screen, at its commit of `commitKey` (`reviewCommitKey`) of the whole
 * stack. A branch is gone only from the table the window holds whole, as `gone` of the review: a press before the
 * window's first view, or while the session builds the branches, waits for it
 */
async function reviewBranchOf(name: string, commitKey: string | undefined): Promise<boolean> {
  await whenTableSettled()
  const branch = getBranch(name)
  if (branch === undefined) {
    notifyError(`Cannot review ${name}`, branchGoneReason(name))
    return false
  }
  const commit = commitKey === undefined ? undefined : branch.commits.find(it => reviewCommitKey({ ...it, subject: it.strippedSubject }) === commitKey)
  return (commit === undefined ? null : reviewBranchCommit(name, commit.originalHash, null)) ?? openBranchReviewWindow(name)
}

/**
 * "⋯" of the title bar: the whole menu of the row of the branch (of the whole stack, of the kept branch), each item run
 * where its action runs (`useReviewActions`); disabled once the branch left
 */
function BranchReviewMenu({ name, groups, goneReason }: { name: string; groups: MenuItemGroups; goneReason: string | null }) {
  if (groups.length === 0) {
    return null
  }
  const items: MenuItemGroups = goneReason === null ? groups : groups.map(group => group.map(item => ({ ...item, disabled: true, description: goneReason })))
  return <MoreActionsMenu name={name} menu={<DropdownMenuItems groups={items} />} />
}

/** The actions of the title bar: disabled with the reason once the branch left, but its pull request, which still opens */
function shownItems(items: readonly ReviewActionItem[], goneReason: string | null): readonly ReviewActionItem[] {
  if (goneReason === null) {
    return items
  }
  return items.map(item => (item.url !== undefined ? item : { ...item, disabled: true, description: goneReason }))
}

/**
 * The actions on the branch in the title bar: O opens its pull request, Create Pull Request… opens its dialog in this
 * window, S its Safe Push (Push to master, Merge #7 by the route of the repository, its popover at the row of the main
 * window), "Push Branch" without a key (P is the previous change). Their keys work in the whole window, not while the
 * dialog is open. Answers how a click runs one.
 */
function useBranchReviewActions(items: readonly ReviewActionItem[], keysSuspended: boolean): (item: ReviewActionItem) => void {
  const run = (item: ReviewActionItem) => {
    if (item.url !== undefined) {
      openPullRequestUrl(item.url)
    } else {
      item.onSelect(FROM_MENU)
    }
  }
  const handleKey = useEffectEvent((event: KeyboardEvent) => {
    if (keysSuspended) {
      return
    }
    // the physical keys, as in the branch preview: the key an item names
    const item = keyedAction(items, event)
    if (item !== undefined) {
      event.preventDefault()
      if (item.disabled !== true) {
        run(item)
      }
    }
  })
  // the whole window: the focus may be on the page itself
  useEffect(() => {
    const listener = (event: KeyboardEvent) => handleKey(event)
    window.addEventListener("keydown", listener)
    return () => window.removeEventListener("keydown", listener)
  }, [])
  return run
}

/** Why the actions of the review stay disabled: the branch left the table, a kept branch was reapplied or deleted */
function goneReason(branchName: string, kept: KeptReview | undefined): string {
  return kept === undefined ? branchGoneReason(branchName) : keptGoneReason(kept, branchName)
}

/**
 * The review of a commit of a list: the commits of the list above its files (⌥↑/⌥↓), the title bar names the list and
 * the commit, Inline/Unified/Split on its right; no action and no "⋯". It reads only the repository: the commits don't change,
 * so the list it got stays true; nothing is edited here. A move (an open of a commit of the list on screen) shows it.
 */
function CommitsReviewWindow({ data, back, onPlace }: { data: ShownCommitsReview; back: ReactNode; onPlace: ReportPlace }) {
  const items = commitItems(data.commits)
  const [selected, setSelected] = useState(data.commits[data.index]?.id ?? data.commits[0]?.id ?? "")
  const { footer, lastPath, showPosition } = useReviewPosition(data.path)
  const [movedAt, setMovedAt] = useState(data.openedAt)
  if (data.openedAt !== movedAt) {
    setMovedAt(data.openedAt)
    const commit = data.commits[data.index]
    if (data.move && commit !== undefined) {
      setSelected(commit.id)
    }
  }
  const index = data.commits.findIndex(it => it.id === selected)
  useEffect(() => {
    onPlace({ kind: "commits", index: index === -1 ? data.index : index, path: lastPath })
  })

  // the native title names the commit shown, the title bar the list and the commit
  const commit = data.commits[index]
  useWindowTitle(commit === undefined ? null : reviewWindowTitle({ ...data, index }))
  // the plan of the commit (openspec/specs/branch/review-units/spec.md): its overview and the story of its diffs, unless the repository
  // turns the plan off
  const subject = { kind: "commit", id: selected } as const
  const plan = useSubjectPlan(data.repositoryPath, subject)

  return (
    <WindowPage
      data-testid="review-window"
      body="fill"
      readyByContent
      title={
        <>
          {back}
          <WindowTitle parts={[...(data.listLabel === null ? [] : [data.listLabel]), ...(commit === undefined ? [] : [{ text: formatShortHash(commit.id), mono: true }])]} />
        </>
      }
      titleEnd={
        <>
          <ReviewSidebarToggle />
          <DiffViewToggle />
        </>
      }
      footer={
        <PreviewDialogFooter
          className="border-t-0 px-1 py-0"
          data-testid="review-window-footer"
          start={
            <span className="tabular-nums" data-testid="review-position">
              {footer(items, selected)}
            </span>
          }
        >
          <ReviewKeyHints commits={!isSingleCommit(items)} escape="Close" />
        </PreviewDialogFooter>
      }
    >
      <Card className="min-h-0 flex-1 flex-row gap-0 overflow-hidden py-0">
        <div className="min-w-0 flex-1">
          <RangeReview
            items={items}
            selected={selected}
            onSelect={setSelected}
            subject={subject}
            initialPath={lastPath}
            onPositionChange={showPosition}
            rangeNoun="commit"
            plan={plan.plan === null ? null : plan}
            reviewFacts={planFacts(plan)}
          />
        </div>
      </Card>
    </WindowPage>
  )
}

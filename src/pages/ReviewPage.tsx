import { Suspense, lazy, useEffect, useEffectEvent, useRef, useState } from "react"
import type { ReactNode } from "react"
import { useStore } from "zustand"
import { Card } from "@/components/ui/card"
import { BranchReviewActions, BranchReviewBar, BranchReviewBody, BranchReviewGone, BranchReviewMarkReviewed } from "@/components/review/BranchReview"
import type { BranchReviewMoves } from "@/components/review/BranchReview"
import { RangeReview } from "@/components/review/RangeReview"
import { ReviewBackButton } from "@/components/review/ReviewBackButton"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { ReviewPlanCard } from "@/components/review/ReviewPlanCard"
import { ReviewSidebarToggle } from "@/components/review/ReviewSidebarToggle"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { WindowTitle } from "@/components/shared/WindowTitleBar"
import { hasCommitKeys, keyedAction, useBranchReview } from "@/hooks/useBranchReview"
import type { BranchReviewModel } from "@/hooks/useBranchReview"
import { askMainWindow, useLinkedFeedback, useLinkedState } from "@/hooks/useMainWindowLink"
import { planFacts, useSubjectPlan } from "@/hooks/useReviewPlan"
import { useSessionFactStores } from "@/hooks/useSessionFactStores"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { useWindowTitle } from "@/hooks/useWindowTitle"
import { branchGoneReason, keptGoneReason, sameReviewTable, settledKeptTable, settledReviewTable } from "@/lib/branch-review"
import type { ReviewedTable } from "@/lib/branch-review"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import type { MenuItemGroups } from "@/lib/menu"
import { isMac } from "@/lib/platform"
import { openPullRequestUrl } from "@/lib/pull-request"
import { commitItems, isSingleCommit, rangePosition } from "@/lib/range-review"
import { repositoryName } from "@/lib/repository-config"
import type { ReviewPosition } from "@/lib/review-position"
import { formatReviewPosition } from "@/lib/review-position"
import { pushReviewHistory, restoredReview, reviewSubjectName, reviewWindowTitle, shownReviewOf } from "@/lib/review-reuse"
import type { ReviewPlace } from "@/lib/review-reuse"
import { showToast } from "@/lib/toast-message"
import { formatWholeStack } from "@/lib/whole-stack"
import { REVIEW_WINDOW } from "@/lib/window-label"
import { ensureSession, viewVersionKey } from "@/model/store"
import { archivedBranchesStore } from "@/stores/archived-branches"
import { branchSyncStore } from "@/stores/branch-sync"
import { unappliedBranchesStore } from "@/stores/unapplied-branches"
import type {
  BranchReviewFeedback,
  BranchReviewItem,
  BranchReviewMenuItem,
  BranchReviewRequest,
  BranchReviewRequestItem,
  BranchReviewWindowData,
  CommitsReviewWindowData,
  KeptReview,
  ReviewWindowData,
} from "@/types/sub-window"

const WindowCreatePullRequestDialog = lazy(() => import("@/components/review/WindowCreatePullRequestDialog").then(it => ({ default: it.WindowCreatePullRequestDialog })))

/** Where a review of the window says it is, by the start of the review: Back restores it there */
type ReportPlace = (place: ReviewPlace) => void

/**
 * The review window (spec/review/review-window.spec.md#one-window), one for every subject: a branch with its stack, the
 * whole stack of a root, a kept branch (`BranchReview` on the canvas of the window), or a list of commits (the commits
 * above the files, `RangeReview`). An open of the review on screen keeps it (a move shows its commit); a new start
 * (`startedAt`) shows the new subject and keeps the one it left in the history of the window, at the place it was:
 * Back (⌘[, the button at the start of the title bar) starts that one over. The window subscribes to the session of its
 * repository for the heads a sync built and the baseline.
 */
export function ReviewPage() {
  const incoming = useSubWindowData<ReviewWindowData>()
  // the open last taken, the review on screen, the reviews it left (the latest last)
  const [taken, setTaken] = useState(incoming)
  const [shown, setShown] = useState(incoming)
  const [history, setHistory] = useState<readonly ReviewWindowData[]>([])
  if (incoming !== taken) {
    setTaken(incoming)
    if (incoming !== null) {
      if (shown !== null && incoming.startedAt !== shown.startedAt) {
        setHistory(pushReviewHistory(history, shown))
      }
      setShown(incoming)
    }
  }
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
    // the main window sends the actions of what the window shows now
    void askMainWindow<BranchReviewRequest>(REVIEW_WINDOW, {
      repositoryPath: restored.repositoryPath,
      branchName: restored.kind === "branch" ? (restored.chain.at(-1) ?? "") : "",
      shows: shownReviewOf(restored),
    })
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
  return shown.kind === "commits" ? (
    <CommitsReviewWindow key={key} data={shown} back={backButton} onPlace={reportPlace} />
  ) : (
    <BranchReviewWindow key={key} data={shown} back={backButton} onPlace={reportPlace} />
  )
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
 * the window asks the main window to open it, as the menu of the row would
 */
function useReappliedReview(data: BranchReviewWindowData, gone: boolean): void {
  const kept = data.kept
  const branchName = data.chain[0] ?? ""
  const rebuilt = useStore(branchSyncStore, state => kept !== undefined && gone && state.branches[branchName] !== undefined)
  const [asked, setAsked] = useState(false)
  if (rebuilt && !asked) {
    setAsked(true)
  }
  useEffect(() => {
    if (asked) {
      void askMainWindow<BranchReviewRequest>(REVIEW_WINDOW, { repositoryPath: data.repositoryPath, branchName, review: "branch" })
    }
  }, [asked, data.repositoryPath, branchName])
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
 * The review of a branch (`BranchReview` on the canvas of the window): the stack bar, Unified/Split and the actions on
 * the branch of the part in the title bar, the commits and the owner chips above the files beside the diffs in the
 * island, the position and the keys in the footer. The heads a sync built, the baseline and the branch of each commit
 * of a whole stack come from the window's own subscription to the session of its repository; the window doesn't own
 * the branches: the main window sends the actions of each branch over the link of the window, and runs what it asks
 * for.
 */
function BranchReviewWindow({ data, back, onPlace }: { data: BranchReviewWindowData; back: ReactNode; onPlace: ReportPlace }) {
  // what the main window says of the chain: the actions of each branch
  const linked = useLinkedState(REVIEW_WINDOW, data.state)
  // the window's view of the session: the heads the last sync built, the baseline
  const wholeStack = data.wholeStack === true
  const table = useReviewedTable(data.chain, wholeStack, data.kept)
  const viewVersion = useStore(branchSyncStore, state => viewVersionKey(state.viewVersion))
  // the review plan, as the last sync of the window's session read `branchdeck.reviewExperimental`
  const reviewExperimental = useStore(branchSyncStore, state => state.reviewExperimental)
  // the "Create pull request" dialog of a branch of the chain: in this window, its context from the main window; a move
  // to a commit waits until it closes
  const [dialog, setDialog] = useState<{ branchName: string; generation: number } | null>(null)
  // the reviewer groups of the check that answered while the window is open, else those of the open
  const groups = linked.selected ? linked.groups : null
  const model = useBranchReview(
    { ...data, baselineBranch: table?.baselineBranch ?? data.baselineBranch, groups: groups === null ? data.groups : [...groups], reviewExperimental },
    data.focus,
    { known: table !== null, tips: table?.tips ?? {} },
    { openedAt: data.openedAt, focus: data.move ? data.focus : null, held: dialog !== null },
  )
  // the actions of each branch while the main window showed the repository and had it: they stay (those that need the
  // main window disabled) while it shows another one, and for a branch that left
  const [seenItems, setSeenItems] = useState(data.state.items)
  const [knownItems, setKnownItems] = useState(data.state.items)
  if (linked.selected && linked.items !== seenItems) {
    setSeenItems(linked.items)
    setKnownItems({ ...knownItems, ...linked.items })
  }
  useLinkedFeedback<BranchReviewFeedback>(REVIEW_WINDOW, feedback => {
    if (feedback.kind === "error") {
      notifyError(feedback.title, feedback.message)
      return
    }
    const url = feedback.pullRequestUrl
    showToast({ ...feedback.toast, ...(url !== null && { action: { children: "Open Pull Request", onClick: () => openPullRequestUrl(url) } }) })
  })
  // loaded on the first open, then kept mounted (the dialog animates and returns the focus when it closes)
  const [isDialogLoaded, setDialogLoaded] = useState(false)
  if (dialog !== null && !isDialogLoaded) {
    setDialogLoaded(true)
  }
  // the review of the whole stack asks for the actions, the menu and the dialog of the whole stack of its root
  const askDialog = (branchName: string, open: boolean) =>
    void askMainWindow<BranchReviewRequest>(REVIEW_WINDOW, {
      repositoryPath: data.repositoryPath,
      branchName,
      ...(wholeStack && { wholeStack }),
      dialog: open ? "open" : "close",
    })
  const openDialog = (branchName: string) => {
    setDialog({ branchName, generation: (dialog?.generation ?? 0) + 1 })
    askDialog(branchName, true)
  }
  const closeDialog = () => {
    if (dialog !== null) {
      askDialog(dialog.branchName, false)
    }
    setDialog(null)
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
  // up to the whole stack and down to a branch of it: the main window opens the review as its menus would
  const moves: BranchReviewMoves = {
    wholeStack: linked.wholeStack,
    onWholeStack: () => void askMainWindow<BranchReviewRequest>(REVIEW_WINDOW, { repositoryPath: data.repositoryPath, branchName: target, scope: "wholeStack" }),
    from: data.from ?? [],
    stackCommits: table?.stackCommits ?? noStackCommits,
    commitMenus: linked.commitMenus,
    onCommitMenuItem: (branchName, commitId, commitMenuItemId) =>
      void askMainWindow<BranchReviewRequest>(REVIEW_WINDOW, { repositoryPath: data.repositoryPath, branchName, commitMenuItemId, commitId }),
    onBranch: (branchName, commitKey) =>
      void askMainWindow<BranchReviewRequest>(
        REVIEW_WINDOW,
        commitKey === undefined
          ? { repositoryPath: data.repositoryPath, branchName, review: "branch" }
          : { repositoryPath: data.repositoryPath, branchName, review: "commit", commitKey },
      ),
  }
  const actions = useBranchReviewActions(
    data.repositoryPath,
    loaded?.target ?? "",
    loaded === null
      ? []
      : shownItems(knownItems[loaded.target] ?? [], {
          repositoryPath: data.repositoryPath,
          branchName: loaded.target,
          selected: linked.selected,
          gone: model.gone.includes(loaded.target),
          kept: data.kept,
        }),
    { openCreatePullRequest: openDialog, keysSuspended: dialog !== null, wholeStack, kept: data.kept },
  )
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
        </>
      }
      titleEnd={
        loaded !== null && (
          <>
            <ReviewSidebarToggle />
            <DiffViewToggle />
            <div className="flex items-center gap-1" data-testid="branch-review-actions">
              <BranchReviewMarkReviewed model={model} />
              <BranchReviewActions actions={actions.items} branchName={loaded.target} onRun={actions.run} />
              <BranchReviewMenu
                repositoryPath={data.repositoryPath}
                branchName={loaded.target}
                groups={linked.menus?.[loaded.target] ?? []}
                selected={linked.selected}
                gone={model.gone.includes(loaded.target)}
                wholeStack={wholeStack}
                kept={data.kept}
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
        <div className="min-w-0 flex-1">
          <BranchReviewBody model={model} moves={moves} />
        </div>
      </Card>
      {/* outside the review: its keys never see what is typed into the dialog */}
      {isDialogLoaded && (
        <Suspense fallback={null}>
          <WindowCreatePullRequestDialog repositoryPath={data.repositoryPath} dialog={dialog} wholeStack={wholeStack} context={linked.createPullRequest} onClose={closeDialog} />
        </Suspense>
      )}
    </WindowPage>
  )
}

const noStackCommits: Readonly<Record<string, string>> = {}

/**
 * "⋯" of the title bar: the whole menu of the branch's row, as the main window sends it; the main window runs the item
 * (it focuses itself and shows the row: a rename or an amend is a form of the row; it switches to this repository first
 * while it shows another one). Disabled once the branch left, as the actions it runs.
 */
function BranchReviewMenu({
  repositoryPath,
  branchName,
  groups,
  selected,
  gone,
  wholeStack,
  kept,
}: {
  repositoryPath: string
  branchName: string
  groups: readonly (readonly BranchReviewMenuItem[])[]
  selected: boolean
  gone: boolean
  wholeStack: boolean
  kept: KeptReview | undefined
}) {
  if (groups.length === 0) {
    return null
  }
  const items: MenuItemGroups = groups.map(group =>
    group.map(item => ({
      ...item,
      ...(gone && { disabled: true, description: goneReason(branchName, kept) }),
      ...(!gone && !selected && item.disabled !== true && { description: switchesRepository(repositoryPath) }),
      onSelect: () =>
        void askMainWindow<BranchReviewRequest>(REVIEW_WINDOW, {
          repositoryPath,
          branchName,
          ...(wholeStack && { wholeStack }),
          ...(kept !== undefined && { kept }),
          menuItemId: item.id,
        }),
    })),
  )
  return <MoreActionsMenu name={wholeStack ? formatWholeStack(branchName) : branchName} menu={<DropdownMenuItems groups={items} />} />
}

/**
 * The actions of the footer as the main window last sent them: those it runs are disabled with the reason once the
 * branch left; while it shows another repository they stay, and their hover says that the main window switches to this
 * one first (`showRepository`); a pull request opens from here anyway
 */
function shownItems(
  items: readonly BranchReviewItem[],
  { repositoryPath, branchName, selected, gone, kept }: { repositoryPath: string; branchName: string; selected: boolean; gone: boolean; kept: KeptReview | undefined },
): readonly BranchReviewItem[] {
  if (selected && !gone) {
    return items
  }
  return items.map(item => {
    if (item.url !== undefined || item.id === "open-pull-request" || item.id === "open-stack-pull-request") {
      return item
    }
    return gone ? { ...item, disabled: true, description: goneReason(branchName, kept) } : { ...item, description: switchesRepository(repositoryPath) }
  })
}

/**
 * The actions on the branch in the title bar, the items of its menu: O opens its pull request, Create Pull Request…
 * opens its dialog in this window, S asks the main window for its Safe Push (Push to master, Merge #7 by the route of
 * the repository), "Push Branch" to push it, without a key
 * (P is the previous change). Their keys work in the whole window, not while the dialog is open. The window doesn't
 * own the branches: the main window runs what it asks for and says why it can't.
 */
function useBranchReviewActions(
  repositoryPath: string,
  branchName: string,
  items: readonly BranchReviewItem[],
  {
    openCreatePullRequest,
    keysSuspended,
    wholeStack,
    kept,
  }: { openCreatePullRequest: (branchName: string) => void; keysSuspended: boolean; wholeStack: boolean; kept: KeptReview | undefined },
) {
  const ask = (itemId: BranchReviewRequestItem) => void askMainWindow<BranchReviewRequest>(REVIEW_WINDOW, { repositoryPath, branchName, ...(wholeStack && { wholeStack }), itemId })
  const run = (item: BranchReviewItem) => {
    if (kept !== undefined) {
      // the actions of a kept branch are items of its menu, which the main window runs
      void askMainWindow<BranchReviewRequest>(REVIEW_WINDOW, { repositoryPath, branchName, kept, menuItemId: item.id })
    } else if (item.url !== undefined) {
      openPullRequestUrl(item.url)
    } else if (item.id === "create-pull-request" || item.id === "create-stack-pull-request") {
      openCreatePullRequest(branchName)
    } else if (item.id !== "open-pull-request" && item.id !== "open-stack-pull-request" && item.id !== "reapply") {
      ask(item.id)
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
  return { items, run }
}

/** Why the actions of the review stay disabled: the branch left the table, a kept branch was reapplied or deleted */
function goneReason(branchName: string, kept: KeptReview | undefined): string {
  return kept === undefined ? branchGoneReason(branchName) : keptGoneReason(kept, branchName)
}

/** The hover of an action of the window while the main window shows another repository: the consequence of the click */
function switchesRepository(repositoryPath: string): string {
  return `Switches the main window to ${repositoryName(repositoryPath)}`
}

/**
 * The review of a commit of a list: the commits of the list above its files (⌥↑/⌥↓), the title bar names the list and
 * the commit, Unified/Split on its right; no action and no "⋯". It reads only the repository: the commits don't change,
 * so the list it got stays true; nothing is edited here. A move (an open of a commit of the list on screen) shows it.
 */
function CommitsReviewWindow({ data, back, onPlace }: { data: CommitsReviewWindowData; back: ReactNode; onPlace: ReportPlace }) {
  const items = commitItems(data.commits)
  const [selected, setSelected] = useState(data.commits[data.index]?.id ?? data.commits[0]?.id ?? "")
  const [position, setPosition] = useState<ReviewPosition | null>(null)
  const [lastPath, setLastPath] = useState(data.path)
  const [movedAt, setMovedAt] = useState(data.openedAt)
  if (data.openedAt !== movedAt) {
    setMovedAt(data.openedAt)
    const commit = data.commits[data.index]
    if (data.move && commit !== undefined) {
      setSelected(commit.id)
    }
  }
  const showPosition = (next: ReviewPosition) => {
    setPosition(next)
    if (next.path !== null) {
      setLastPath(next.path)
    }
  }
  const index = data.commits.findIndex(it => it.id === selected)
  useEffect(() => {
    onPlace({ kind: "commits", index: index === -1 ? data.index : index, path: lastPath })
  })

  // the native title names the commit shown, the title bar the list and the commit
  const commit = data.commits[index]
  useWindowTitle(commit === undefined ? null : reviewWindowTitle({ ...data, index }))
  // the plan of the commit (spec/branch/review-units.spec.md): its card and the story of its diffs, unless the repository
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
              {formatReviewPosition({ file: position, ...rangePosition(items, selected) })}
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
            commitTop={plan.plan === null ? undefined : <ReviewPlanCard state={plan} />}
            reviewFacts={planFacts(plan)}
          />
        </div>
      </Card>
    </WindowPage>
  )
}

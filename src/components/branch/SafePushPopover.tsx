import { useEffect, useRef, useState } from "react"
import type { ReactNode, RefObject } from "react"
import { cn } from "cn"
import {
  ArrowDownToLineIcon,
  CircleCheckIcon,
  ExternalLinkIcon,
  FileDiffIcon,
  FlaskConicalIcon,
  InfoIcon,
  Layers2Icon,
  SettingsIcon,
  ShieldCheckIcon,
  TriangleAlertIcon,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent } from "@/components/ui/popover"
import { Select, SelectContent, SelectGroup, SelectItem, SelectLabel, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { moveCommits } from "@/hooks/useMoveCommits"
import type { SafePushRead } from "@/hooks/useSafePush"
import { pushAlone, requestSafePush, useSafePushRead } from "@/hooks/useSafePush"
import { restackBranch } from "@/hooks/useStackActions"
import { useStackIndependence } from "@/hooks/useStackIndependence"
import { useStackNextStep } from "@/hooks/useStackNextStep"
import { getWholeStackStartItems } from "@/hooks/useWholeStackActions"
import type { Commit, LandProfiles, PatronusReviewRequirement, Person, PullRequest, ReviewReadiness, StackCommit } from "@/lib/bindings"
import { openBranchReviewWindow, openWholeStackReviewWindow, toReviewGroups } from "@/lib/branch-review-window"
import { isRulesUnknown } from "@/lib/branch-rules"
import { formatCommitCount } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import type { LandingDecision } from "@/lib/landing-gate"
import { describeLandingCheck, formatLandingFix } from "@/lib/landing-gate"
import type { MenuItem } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { formatPushMode, formatReviewerGroup, formatReviewerGroupFiles } from "@/lib/patronus"
import { openMergeRequest } from "@/lib/patronus-merge-request"
import type { StackAlone } from "@/lib/patronus-stack"
import { describeStackLanding } from "@/lib/patronus-stack"
import { startSafePush } from "@/lib/safe-push-start"
import { openSettings } from "@/lib/settings-window"
import { joinNames } from "@/lib/text"
import {
  describeStackBranchPush,
  describeWholeStackApprovalsForPiece,
  describeWholeStackLanding,
  formatReadinessSubject,
  formatWholeStackApprovals,
  stackRootOfReadinessKey,
} from "@/lib/whole-stack"
import { landingRefusalsOf } from "@/presenters/land"
import { formatPerson } from "@/presenters/next-step"
import { getBranch, useBranchSync, wholeStackRootIn } from "@/stores/branch-sync"
import { useIndicatorClaim, useObjectBusy } from "@/stores/operations"
import { peekBranchRules, usePullRequest, useStackPullRequest } from "@/stores/pull-requests"
import type { SafePushAsk, SafePushMode, SafePushPlacement } from "@/stores/safe-push"
import { closeSafePushPopover } from "@/stores/safe-push"
import { pinStackPath } from "@/stores/stack-view"

/** The Select value of the default checks of the target: no merge profile */
const DEFAULT_CHECKS = "\0default"

/**
 * The commits of the branch list that `commits` name, for a move: each in its branch, by its original hash. The popover
 * offers the move only while the list has every one of them
 */
function syncedCommits(commits: readonly StackCommit[]): Commit[] {
  return commits.flatMap(commit => {
    const branch = getBranch(commit.branch)
    const index = branch?.commitIndex[commit.id]
    const synced = index === undefined ? undefined : branch?.commits[index]
    return synced === undefined ? [] : [synced]
  })
}

/**
 * The popover of a start that has something to say before it starts (`presentLand` of the plan Rust answers, `prepare`:
 * S, and each of `Plan.confirmations`: a stack, a profile other than the default checks, a running Safe Push it cancels,
 * approvals Patronus judges only at the start, a push that dismisses approvals, a start without review, a branch its
 * build needs), at the slot it was asked from (`anchor`). Its title is what lands: "Lands
 * oauth with auth and login, which it is built on: 3 branches · 12 commits into master."; below it only what applies,
 * in sentences: why its build fails (the landing needs, with the fix; its start is Start Anyway), the pull request it comments on, the branches that stay, the profile (the branch's own named as the last
 * Safe Push's), the review as Patronus answered it, the running Safe Push it cancels, the second way to land a stacked
 * branch alone (the unstack, the move of what it needs; offered, never decided), and of the root of a stack or a branch
 * of one that forks the whole stack in one robot (`WholeStackOption`). The footer says what it waits for or
 * why it can't start, then Dry Run, Cancel and the start, which takes Enter once it can run. A start closes it: the
 * slot shows the push and the start.
 */
export function SafePushPopover({ ask, anchor }: { ask: SafePushAsk; anchor: RefObject<HTMLElement | null> }) {
  const { branchName, mode } = ask
  // the whole stack of a root (`stackReadinessKey`): its stack branch, its pull request
  const root = stackRootOfReadinessKey(branchName)
  const subject = formatReadinessSubject(branchName)
  const [chosenProfile, setChosenProfile] = useState<string | null | undefined>(ask.profileId)
  const read = useSafePushRead(ask, chosenProfile)
  const stacked = read !== null && read.ancestors.length > 0
  const independence = useStackIndependence(branchName, mode === "rebaseAutosquash" && stacked)
  // a branch of a stack: the whole stack lands in one Safe Push too, the second way its popover offers
  const wholeStackRoot = useBranchSync(state => (root === null ? wholeStackRootIn(state, branchName) : null))
  const wholeStackNext = useStackNextStep(wholeStackRoot ?? "")
  const wholeStackItem =
    wholeStackRoot === null
      ? undefined
      : getWholeStackStartItems(wholeStackRoot, wholeStackNext).find(item => item.id === (mode === "dryRun" ? "dry-run-whole-stack" : "safe-push-whole-stack"))
  const branchPullRequest = usePullRequest(branchName)
  const stackPullRequest = useStackPullRequest(root ?? "")
  const pullRequest = root === null ? branchPullRequest : stackPullRequest
  // the unstack and the move it offers run from here (its status line shows them); any operation on the branch holds the start
  const surface = `safe-push-popover:${branchName}`
  useIndicatorClaim(surface)
  const busy = useObjectBusy(branchName)
  const startRef = useRef<HTMLButtonElement>(null)
  const popupRef = useRef<HTMLDivElement>(null)

  // the stack graph shows what the start of a branch lands while the popover is open; the whole stack is all of it
  useEffect(() => {
    if (root !== null) {
      return
    }
    pinStackPath(branchName, true)
    return () => pinStackPath(branchName, false)
  }, [branchName, root])

  const decision = read?.prompt ?? { kind: "wait" as const, what: `Reading ${subject}…` }
  const canStart = (decision.kind === "start" || decision.kind === "confirm") && busy === null
  // a start enabled later (its facts loaded): Enter starts it, unless the user moved to a control of the popover; after
  // the frame in which Base UI moves the focus into the popup, which it does after the first effects
  const canStartRef = useRef(canStart)
  useEffect(() => {
    canStartRef.current = canStart
    if (!canStart) {
      return
    }
    const frame = requestAnimationFrame(() => {
      const active = document.activeElement
      const popup = popupRef.current
      if (popup !== null && (active === popup || active === document.body || active === null || !popup.contains(active))) {
        startRef.current?.focus()
      }
    })
    return () => cancelAnimationFrame(frame)
  }, [canStart])

  const answered = independence.state.status === "loaded" ? independence.state.independence : null
  const neededCommits = answered?.kind === "needs" ? answered.parentCommits : []
  // the check reads HEAD, the branch list the last sync: a move takes what the list has, so it waits for every commit
  const neededInList = useBranchSync(state => neededCommits.every(commit => state.branches[commit.branch]?.commitIndex[commit.id] !== undefined))

  const modeName = formatPushMode(mode)
  // the build of what it lands needs a branch outside it (the landing gate's blockers): the reason, the fix, and the start
  // anyway
  const landingRefusals = read?.plan == null ? [] : landingRefusalsOf(read.plan).map(reason => reason.refusal)
  const landingCheck = landingRefusals.length === 0 ? null : describeLandingCheck({ refusals: landingRefusals }, subject, {})
  const landingRefused = landingCheck?.kind === "refuse" ? landingCheck : null
  const close = () => closeSafePushPopover(branchName)
  const start = () => {
    if (read === null || !canStart) {
      return
    }
    close()
    void startSafePush(landingRefused === null ? read.start : { ...read.start, allow: landingRefused.anyway })
  }
  const dryRun = () => {
    close()
    void requestSafePush(branchName, { mode: "dryRun", via: "click", placement: ask.placement })
  }
  /** The unstack, with its sync: the popover reads the branch alone then, with its review; no start */
  const unstack = () => void restackBranch(branchName, null, { surface })
  /** The move of the needed commits into the branch, then the check again */
  const moveHere = (commits: readonly StackCommit[]) =>
    void moveCommits(syncedCommits(commits), branchName, { surface }).then(moved => {
      if (moved) {
        independence.recheck()
      }
    })

  const target = read?.target ?? ""
  const landing =
    read === null
      ? null
      : describeStackLanding({
          branchName,
          mode,
          target,
          ancestors: read.ancestors,
          commitCount: read.commitCount,
          children: read.children,
          independence: answered?.kind === "needs" && !neededInList ? { ...answered, movable: false } : answered,
        })
  const commits = read?.commitCount ?? 0
  const wholeStack = read?.wholeStack ?? null
  const title =
    (wholeStack === null ? null : `${describeWholeStackLanding(wholeStack.members, commits, target, mode, wholeStack.landed)}.`) ??
    landing?.lands ??
    (mode === "dryRun"
      ? `Checks ${branchName} on ${target} without pushing into it: ${formatCommitCount(commits)}.`
      : `Lands ${branchName} into ${target}: ${formatCommitCount(commits)}.`)

  return (
    <Popover open onOpenChange={open => !open && close()}>
      <PopoverContent
        ref={popupRef}
        anchor={anchor}
        align="end"
        collisionPadding={16}
        className="w-[28rem] max-w-[calc(100vw-2rem)] gap-3 p-3"
        // read when Base UI moves the focus in, which may be after the facts loaded
        initialFocus={() => (canStartRef.current ? startRef.current : popupRef.current)}
        aria-label={`${modeName} of ${subject}`}
        data-testid="safe-push-popover"
        data-mode={mode}
        data-decision={decision.kind}
        onKeyDown={event => {
          // Enter on the popup itself (not on a control) starts, as the focused start does
          if (event.key === "Enter" && event.target === popupRef.current) {
            event.preventDefault()
            start()
          }
        }}
      >
        {/* not the popup's name: that is "Safe Push of auth"; the title is what it lands */}
        <h2 className="text-sm font-medium" data-testid="safe-push-lands">
          {title}
        </h2>
        {landingRefused !== null && <LandingNeedsLine refused={landingRefused} disabled={busy !== null} surface={surface} />}
        {read !== null && <Lines read={read} landing={landing} pullRequest={pullRequest} onProfile={setChosenProfile} />}
        {landing?.alone != null && read !== null && <AloneOption alone={landing.alone} disabled={busy !== null} onUnstack={unstack} onMove={moveHere} />}
        {wholeStackItem !== undefined && wholeStackRoot !== null && <WholeStackOption root={wholeStackRoot} item={wholeStackItem} mode={mode} onSelect={close} />}
        {independence.state.status === "loading" && (
          <p className="flex items-center gap-2 text-muted-foreground" data-testid="safe-push-alone-checking">
            <Spinner className="size-4" />
            {`Checking whether ${branchName} builds alone…`}
          </p>
        )}
        <div className="flex flex-wrap items-center justify-end gap-2 pt-1">
          <Status read={read} busy={busy} placement={ask.placement} onPiece={close} />
          {mode === "rebaseAutosquash" && (
            <Button size="xs" variant="ghost" onClick={dryRun} data-testid="safe-push-dry-run">
              <FlaskConicalIcon data-icon="inline-start" />
              Dry Run
            </Button>
          )}
          <Button size="xs" variant="ghost" onClick={close}>
            Cancel
          </Button>
          <Button ref={startRef} size="xs" disabled={!canStart} focusableWhenDisabled className={cn(!canStart && "opacity-50")} onClick={start} data-testid="safe-push-start">
            {mode === "dryRun" ? <FlaskConicalIcon data-icon="inline-start" /> : <ShieldCheckIcon data-icon="inline-start" />}
            {landingRefused !== null ? "Start Anyway" : read?.withoutReview === true ? "Safe Push Without Review" : modeName}
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  )
}

/**
 * Why the build of what the start lands fails (`describeLandingCheck` of the landing needs): the name a branch outside
 * it adds, with the fix as a button ("Stack lane-run on stills-codec"), which restacks and leaves the popover open to
 * read the start again; the start below is Start Anyway
 */
function LandingNeedsLine({ refused, disabled, surface }: { refused: Extract<LandingDecision, { kind: "refuse" }>; disabled: boolean; surface: string }) {
  const fix = refused.fix
  const label = formatLandingFix(fix)
  return (
    <div className="flex flex-col items-start gap-1.5" data-testid="safe-push-landing-needs">
      <Line tone="warning">{refused.reason}</Line>
      {fix?.kind === "restack" && label !== null && (
        <Button size="xs" variant="outline" disabled={disabled} onClick={() => void restackBranch(fix.branch, fix.onto, { surface })} data-testid="safe-push-landing-fix">
          <Layers2Icon data-icon="inline-start" />
          {label}
        </Button>
      )}
    </div>
  )
}

/** A line of the popover: muted by default, `warning` or `info` with its icon */
function Line({ tone = "muted", testId, children }: { tone?: "muted" | "plain" | "warning" | "info" | "success"; testId?: string; children: ReactNode }) {
  const icon =
    tone === "warning" ? (
      <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />
    ) : tone === "info" ? (
      <InfoIcon className="mt-0.5 size-4 shrink-0 text-info" />
    ) : tone === "success" ? (
      <CircleCheckIcon className="mt-0.5 size-4 shrink-0 text-success" />
    ) : null
  return (
    <p className={cn("flex items-start gap-1.5", tone === "warning" ? "text-warning" : tone === "plain" ? "text-foreground" : "text-muted-foreground")} data-testid={testId}>
      {icon}
      <span>{children}</span>
    </p>
  )
}

/**
 * What applies of the start below its title: the pull requests it retargets out of a virtual branch first (D11), the
 * pull request it comments on, the branches that stay, the profile, the review, the running Safe Push it cancels, a push
 * that dismisses approvals or rules that can't be read
 */
function Lines({
  read,
  landing,
  pullRequest,
  onProfile,
}: {
  read: SafePushRead
  landing: ReturnType<typeof describeStackLanding> | null
  pullRequest: PullRequest | undefined
  onProfile: (id: string | null) => void
}) {
  const { fromPullRequest, profiles, plan } = read
  const branchProfile = profiles?.offered.find(profile => profile.id === profiles.default) ?? null
  const confirmations = plan?.confirmations ?? []
  // the push dismisses approvals of the pull request: the ones Patronus counts refuse the start, others it confirms
  const dismisses = confirmations.find(confirmation => confirmation.kind === "dismissesApprovals") ?? plan?.blockers.find(reason => reason.code === "dismissesApprovals")
  const rulesUnknown = confirmations.find(confirmation => confirmation.kind === "rulesUnknown")
  const rules = rulesUnknown === undefined ? null : peekBranchRules(read.target)
  return (
    <>
      {read.start.effects.map(effect =>
        effect.kind === "retarget" ? (
          <Line key={effect.number} testId="safe-push-retarget">
            {`Retargets #${effect.number} of ${effect.branch} from ${effect.from} to ${effect.to} first: every pull request of a virtual branch goes into ${effect.to}.`}
          </Line>
        ) : null,
      )}
      {fromPullRequest !== null && (
        <Line testId="safe-push-from-pull-request">
          {`Comments ${fromPullRequest.command} on #${fromPullRequest.number}: Patronus reports there${read.mode === "dryRun" ? "" : " and closes it once merged"}.`}
          {branchProfile !== null && ` A comment runs the default checks: it can't choose ${branchProfile.name}.`}
        </Line>
      )}
      {/* only while its approvals matter: next to "No code-owner review needed" it would say what doesn't hold */}
      {read.mode === "rebaseAutosquash" &&
        fromPullRequest !== null &&
        read.wholeStack !== null &&
        read.review.kind === "loaded" &&
        read.review.readiness.kind !== "notRequired" && <Line testId="safe-push-whole-stack-approvals">{`${formatWholeStackApprovals(fromPullRequest.number)}.`}</Line>}
      {read.wholeStack !== null && read.start.effects.some(effect => effect.kind === "push") && (
        <Line testId="safe-push-stack-branch">{describeStackBranchPush(read.wholeStack.stackBranch, read.wholeStack.members, read.wholeStack.onRemote)}</Line>
      )}
      {read.wholeStackPullRequest !== null && (
        <Line tone="info" testId="safe-push-whole-stack-pull-request">
          {`${describeWholeStackApprovalsForPiece(read.wholeStackPullRequest.root, read.wholeStackPullRequest.number, read.branchName)}.`}
        </Line>
      )}
      {landing?.children != null && <Line testId="safe-push-children">{landing.children}</Line>}
      {fromPullRequest === null && profiles !== null && (profiles.offered.length > 0 || read.withoutReview) && (
        <ProfileField read={read} profiles={profiles} onChange={onProfile} />
      )}
      <ReviewLine read={read} pullRequest={pullRequest} />
      {read.replaces !== null && (
        <Line tone="warning" testId="safe-push-replaces">
          {`Cancels the ${formatPushMode(read.mode)} of ${read.subject} running on ${formatShortHash(read.replaces.sourceRevision ?? "")}.`}
        </Line>
      )}
      {dismisses !== undefined && (
        <Line tone="warning" testId="safe-push-dismisses">
          {`The push dismisses the approvals of #${dismisses.number}: the rules of the repository dismiss stale reviews when new commits are pushed.`}
        </Line>
      )}
      {rulesUnknown?.kind === "rulesUnknown" && rules !== null && isRulesUnknown(rules) && (
        <Line tone="warning" testId="safe-push-rules-unknown">
          {`Cannot check whether the push keeps the approvals of #${rulesUnknown.number}: ${rules.reason}. Patronus counts them when the robot starts.`}
        </Line>
      )}
    </>
  )
}

/**
 * The merge profile: "Default checks" and the profiles the mode allows, the branch's own named as the last Safe Push's,
 * the description Patronus gives; for a start without review the groups by what Patronus says of them (skips the review,
 * unknown, emergency) and no default checks, the emergency one told in red
 */
function ProfileField({ read, profiles, onChange }: { read: SafePushRead; profiles: LandProfiles; onChange: (id: string | null) => void }) {
  const selected = read.profile
  const bypass = read.withoutReview
  const groups = bypass
    ? [
        { label: "Skip the code-owner review", items: profiles.offered.filter(it => it.skipsReview === true) },
        { label: profiles.offered.some(it => it.skipsReview === true) ? "Other profiles" : "Profiles", items: profiles.offered.filter(it => it.skipsReview === null) },
        { label: "Emergency", items: profiles.emergency },
      ]
        .map(group => ({ label: group.label, items: group.items.map(it => ({ value: it.id, label: it.name })) }))
        .filter(group => group.items.length > 0)
    : [{ label: null, items: [{ value: DEFAULT_CHECKS, label: "Default checks" }, ...profiles.offered.map(it => ({ value: it.id, label: it.name }))] }]
  const items = groups.flatMap(group => group.items)
  const isEmergency = selected !== null && bypass && profiles.emergency.some(it => it.id === selected.id)
  const value = selected?.id ?? (bypass ? null : DEFAULT_CHECKS)
  return (
    <div className="flex flex-col gap-1.5" data-testid="safe-push-profile-field">
      <label className="flex items-center gap-3">
        <span className="text-muted-foreground">Profile</span>
        <Select items={items} value={value} onValueChange={next => onChange(next === DEFAULT_CHECKS ? null : next)}>
          <SelectTrigger size="sm" className="w-64" data-testid="safe-push-profile">
            <SelectValue placeholder="Choose a profile" />
          </SelectTrigger>
          <SelectContent alignItemWithTrigger={false}>
            {groups.map(group =>
              group.label === null ? (
                group.items.map(item => (
                  <SelectItem key={item.value} value={item.value}>
                    {item.label}
                  </SelectItem>
                ))
              ) : (
                <SelectGroup key={group.label}>
                  <SelectLabel>{group.label}</SelectLabel>
                  {group.items.map(item => (
                    <SelectItem key={item.value} value={item.value}>
                      {item.label}
                    </SelectItem>
                  ))}
                </SelectGroup>
              ),
            )}
          </SelectContent>
        </Select>
      </label>
      {selected !== null && selected.id === profiles.default && (
        <Line testId="safe-push-profile-source">{`The profile of the last ${formatPushMode(read.mode)} of ${read.subject}.`}</Line>
      )}
      {selected?.description != null && <Line testId="safe-push-profile-description">{selected.description}</Line>}
      {selected !== null &&
        (isEmergency ? (
          <p className="flex items-start gap-1.5 text-destructive" data-testid="safe-push-profile-note" data-kind="emergency">
            <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />
            {`${selected.name} skips all checks of the Safe Push, not only the code-owner review: Patronus pushes into the target without testing it.`}
          </p>
        ) : (
          bypass && (
            <Line testId="safe-push-profile-note">
              {selected.skipsReview === true
                ? `${selected.name} skips the code-owner review: the last Safe Push of the branch ran with it.`
                : `Patronus tells only when the robot starts whether ${selected.name} skips the code-owner review: if it doesn't, the Safe Push fails at once.`}
            </Line>
          )
        ))}
    </div>
  )
}

/** "Approved by Ilya Korennoy for IJPL Code Platform; by Alice for Air Platform." */
function formatApproved(groups: readonly { name: string; approvedBy: Person[] }[]): string {
  const parts = groups.map(group => (group.approvedBy.length === 0 ? `for ${group.name}` : `by ${joinNames(group.approvedBy.map(formatPerson))} for ${group.name}`))
  return `Approved ${parts.join("; ")}.`
}

/** "Patronus doesn't tell yet whose approvals count: Branch Deck can't check junie-jetbrains's approval." */
function formatUnknown(readiness: Extract<ReviewReadiness, { kind: "unknown" | "unavailable" }>, pullRequest: PullRequest | undefined): string {
  if (readiness.kind === "unavailable") {
    return `Patronus didn't answer the review check (${readiness.message}): the robot checks the approvals when it starts.`
  }
  const approvers = pullRequest?.reviewers.filter(reviewer => reviewer.state === "approved").map(reviewer => reviewer.login) ?? []
  if (approvers.length === 0) {
    return "Patronus doesn't tell yet whose approvals count: the robot checks them when it starts."
  }
  const names = joinNames(approvers)
  return `Patronus doesn't tell yet whose approvals count: Branch Deck can't check ${approvers.length === 1 ? `${names}'s approval` : `the approvals of ${names}`}.`
}

/**
 * The code-owner review of a Safe Push in one line, as Patronus answered it for the branch as it is: approved (whose
 * approvals count), not needed, judged only at the start, missing but skipped by the profile; on Space, the approvals the
 * merge request needs. Nothing for a dry run, nor while it loads (the footer says so)
 */
function ReviewLine({ read, pullRequest }: { read: SafePushRead; pullRequest: PullRequest | undefined }) {
  const { review, profile } = read
  switch (review.kind) {
    case "none":
      return null
    case "unlisted":
      return <Line testId="safe-push-review">GitHub doesn't list the pull requests of the repository: Patronus checks the approvals when the robot starts.</Line>
    case "space": {
      if (!review.required) {
        return null
      }
      const requirement = read.readiness?.requirement ?? null
      const mergeRequest = requirement?.mergeRequest ?? null
      const groups = requirement?.groups.filter(group => group.approved !== true) ?? []
      return (
        <div className="flex flex-col gap-1.5">
          <Line tone="warning" testId="safe-push-review">
            {`Needs review by the code owners: a Safe Push fails without these approvals${mergeRequest === null ? "." : `, which count in ${mergeRequest.reviewKey}.`}`}
            {/* say which system: the approvals count in the GitHub pull request, which can't be opened without its repository */}
            {mergeRequest === null && " They count in the GitHub pull request of the branch, and its GitHub repository is unknown."}
          </Line>
          {groups.length > 0 && (
            <ul className="flex flex-col gap-0.5 pl-5.5 text-foreground" data-testid="safe-push-review-groups">
              {groups.map(group => {
                const files = formatReviewerGroupFiles(group)
                return <li key={group.name}>{files === null ? formatReviewerGroup(group) : `${formatReviewerGroup(group)} · ${files}`}</li>
              })}
            </ul>
          )}
          <div className="flex flex-wrap gap-2">
            {mergeRequest === null ? (
              <Button variant="outline" size="xs" onClick={() => openSettings({ page: "repository", focus: "githubRepository" })} data-testid="safe-push-set-github">
                <SettingsIcon data-icon="inline-start" />
                Set GitHub Repository…
              </Button>
            ) : (
              <Button variant="outline" size="xs" onClick={() => openMergeRequest(mergeRequest.url)}>
                <ExternalLinkIcon data-icon="inline-start" />
                {`Open ${mergeRequest.reviewKey}`}
              </Button>
            )}
            {requirement !== null && <ReviewChanges branchName={read.branchName} requirement={requirement} />}
          </div>
        </div>
      )
    }
    case "loaded": {
      const { readiness } = review
      switch (readiness.kind) {
        case "notRequired":
          return (
            <Line tone="success" testId="safe-push-review">
              No code-owner review needed.
            </Line>
          )
        case "approved":
          return (
            <Line tone="success" testId="safe-push-review">
              {formatApproved(readiness.groups)}
            </Line>
          )
        case "unknown":
        case "unavailable":
          return (
            <Line tone="info" testId="safe-push-review">
              {formatUnknown(readiness, pullRequest)}
            </Line>
          )
        case "missing":
          return (
            <div className="flex flex-col gap-1.5">
              <Line tone="warning" testId="safe-push-review">
                {`Starts without the approvals of ${joinNames(readiness.groups.map(group => group.name))}: ${profile === null ? "the profile you choose has to skip" : `${profile.name} skips`} the code-owner review.`}
              </Line>
              {read.readiness?.requirement != null && <ReviewChanges branchName={read.branchName} requirement={read.readiness.requirement} />}
            </div>
          )
      }
    }
  }
}

/**
 * "Review Changes": the branch in its review window with everything Patronus checks, the reviewer groups as filters; the
 * whole stack in the review window of its stack branch
 */
function ReviewChanges({ branchName, requirement }: { branchName: string; requirement: PatronusReviewRequirement }) {
  const root = stackRootOfReadinessKey(branchName)
  return (
    <Button
      variant="outline"
      size="xs"
      className="w-fit"
      data-testid="safe-push-review-changes"
      onClick={() =>
        void (root === null
          ? openBranchReviewWindow(branchName, { range: "all", groups: toReviewGroups(requirement) })
          : openWholeStackReviewWindow(root, { groups: toReviewGroups(requirement) }))
      }
    >
      <FileDiffIcon data-icon="inline-start" />
      Review Changes
    </Button>
  )
}

/** What the start waits for, why it can't start (the push alone where it helps), or nothing: left of the buttons */
function Status({ read, busy, placement, onPiece }: { read: SafePushRead | null; busy: string | null; placement: SafePushPlacement; onPiece: () => void }) {
  const decision = read?.prompt ?? null
  // an operation on the branch: the unstack or the move started here ("Unstacking auth…"), or one started before
  if (busy !== null) {
    return (
      <p className="mr-auto flex items-center gap-2 text-muted-foreground" data-testid="safe-push-status" aria-busy>
        <Spinner className="size-4" />
        {`${busy}…`}
      </p>
    )
  }
  if (decision === null || decision.kind === "wait") {
    return (
      <p className="mr-auto flex items-center gap-2 text-muted-foreground" data-testid="safe-push-status" data-kind="wait">
        <Spinner className="size-4" />
        {decision?.what ?? "Reading the branch…"}
      </p>
    )
  }
  if (decision.kind === "refuse" || decision.kind === "choose") {
    return (
      <div className="mr-auto flex min-w-0 basis-full flex-col gap-1.5">
        <p
          className={cn("flex items-start gap-1.5", decision.kind === "refuse" ? "text-warning" : "text-muted-foreground")}
          data-testid="safe-push-status"
          data-kind={decision.kind}
        >
          {decision.kind === "refuse" && <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />}
          {decision.reason}
        </p>
        {decision.kind === "refuse" && decision.offersPush && read !== null && <PushAlone branchName={read.branchName} />}
        {decision.kind === "refuse" && read?.wholeStack?.piece != null && <LandInPieces piece={read.wholeStack.piece} mode={read.mode} placement={placement} onSelect={onPiece} />}
      </div>
    )
  }
  return null
}

/**
 * "Land the whole stack: Safe Push Whole Stack": in the popover of a branch of a stack, the start of its whole stack
 * (`getWholeStackStartItems`, the item of the menus), which opens its own popover (at the header of its island, else at
 * the root's row); its hover what it lands. Disabled, it says why under it, as the menus do.
 */
function WholeStackOption({ root, item, mode, onSelect }: { root: string; item: MenuItem; mode: SafePushMode; onSelect: () => void }) {
  const disabled = item.disabled === true
  const button = (
    <Button
      variant="outline"
      size="xs"
      className={cn("w-fit", disabled && "opacity-50")}
      disabled={disabled}
      focusableWhenDisabled
      data-testid="safe-push-whole-stack"
      onClick={() => {
        onSelect()
        item.onSelect(FROM_MENU)
      }}
    >
      {mode === "dryRun" ? <FlaskConicalIcon data-icon="inline-start" /> : <ShieldCheckIcon data-icon="inline-start" />}
      {item.label}
    </Button>
  )
  return (
    <div className="flex flex-col gap-1.5" data-testid="safe-push-whole-stack-option" data-whole-stack={root}>
      <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-muted-foreground">
        {mode === "dryRun" ? "Check the whole stack:" : "Land the whole stack:"}
        {disabled || item.description === undefined ? (
          button
        ) : (
          <Tooltip>
            <TooltipTrigger render={button} />
            <TooltipContent>{item.description}</TooltipContent>
          </Tooltip>
        )}
      </p>
      {disabled && item.description !== undefined && <Line testId="safe-push-whole-stack-reason">{item.description}</Line>}
    </div>
  )
}

/**
 * "Land it in pieces": the whole stack can't land while the sync didn't build its stack branch, the Safe Push of the
 * last branch of its main line can (`WholeStackRead.piece`), its popover at the same slot
 */
function LandInPieces({ piece, mode, placement, onSelect }: { piece: string; mode: SafePushMode; placement: SafePushPlacement; onSelect: () => void }) {
  return (
    <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-muted-foreground" data-testid="safe-push-pieces">
      Land it in pieces:
      <Button
        variant="outline"
        size="xs"
        className="w-fit"
        onClick={() => {
          onSelect()
          void requestSafePush(piece, { mode, via: "click", placement })
        }}
      >
        <ShieldCheckIcon data-icon="inline-start" />
        {`${formatPushMode(mode)} ${piece}`}
      </Button>
    </p>
  )
}

/** The push alone, where it is the step before the start (it dismisses the approvals Patronus counts): ask for the review again after it */
function PushAlone({ branchName }: { branchName: string }) {
  const [pushing, setPushing] = useState(false)
  const push = async () => {
    setPushing(true)
    await pushAlone(branchName)
    setPushing(false)
  }
  return (
    <Button variant="outline" size="xs" className="w-fit" disabled={pushing} onClick={() => void push()} data-testid="safe-push-push-only">
      {pushing ? <Spinner data-icon="inline-start" /> : null}
      Push
    </Button>
  )
}

/** The second way: the branch alone, its reason muted (a use of a name of the stack in `warning`), its action after it */
function AloneOption({ alone, disabled, onUnstack, onMove }: { alone: StackAlone; disabled: boolean; onUnstack: () => void; onMove: (commits: readonly StackCommit[]) => void }) {
  const warning = alone.kind === "unstack" && alone.warning
  return (
    <div className="flex flex-col gap-1.5" data-testid="safe-push-alone" data-action={alone.kind} data-warning={warning || undefined}>
      <Line tone={warning ? "warning" : "muted"}>{alone.text}</Line>
      {alone.kind === "unstack" ? (
        // names, not a compiler, found the use: the unstack stays an offer, a link
        <Button variant={warning ? "link" : "outline"} size="xs" className={warning ? "h-auto w-fit px-0" : "w-fit"} disabled={disabled} onClick={onUnstack}>
          {!warning && <Layers2Icon data-icon="inline-start" />}
          {`Unstack from ${alone.parent}`}
        </Button>
      ) : (
        <Button variant="outline" size="xs" className="w-fit" disabled={disabled} onClick={() => onMove(alone.commits)}>
          <ArrowDownToLineIcon data-icon="inline-start" />
          {alone.label}
        </Button>
      )}
    </div>
  )
}

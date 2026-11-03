import { useEffect, useRef } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { CommitCard } from "@/components/commit/CommitCard"
import { CommitRowMark } from "@/components/commit/CommitRowMark"
import { LinkedText } from "@/components/commit/LinkedText"
import { ContractParagraphView } from "@/components/review/ContractParagraphView"
import { MessageChangeView } from "@/components/review/MessageChangeView"
import { StepFactsLine } from "@/components/review/StepFactsLine"
import { StoryBlock, StoryBody, StoryPart, SubjectBefore } from "@/components/review/StoryBody"
import { Disclosure } from "@/components/shared/Disclosure"
import { ReadingColumn } from "@/components/shared/ReadingColumn"
import type { RowCardState } from "@/components/shared/RowCard"
import { SectionLabel } from "@/components/shared/SectionLabel"
import { useHeadingCard } from "@/hooks/useHeadingCard"
import { useRowCardHover } from "@/hooks/useRowCardHover"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { ReviewCommit } from "@/lib/bindings"
import { toCardCommit } from "@/lib/commit"
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"
import type { ReviewPlanState } from "@/lib/review-plan-answer"
import type { BreakageEntry } from "@/presenters/review-plan"
import type { ContentsStep, PrologueView, StoryView } from "@/presenters/review-story"
import { presentPrologue } from "@/presenters/review-story"
import { getYouTrackKeyOfIssue, loadIssue, useIssueEntry } from "@/stores/youtrack-issues"

/** The commit a review shows alone, at the top of its overview, with the actions of its heading */
export interface PrologueCommit {
  readonly commit: ReviewCommit
  /** At the end of its subject: "Review bazel" of a commit of the whole stack */
  readonly action: ReactNode
  /** The "⋯" of the commit at the end of its meta line */
  readonly menu?: ReactNode
}

/**
 * The overview, the first block of the document of a review (spec/branch/review-tour.spec.md, "In the Review Window";
 * spec/branch/review-plan.spec.md): the headline (the title of the issue the commits name once YouTrack told it, else
 * the name Rust gives the review, the branch as the title bar names it; the issue key a chip beside it; none under the
 * title of a pull request, `headline` false; a commit shown alone its whole heading and body), the body of a pull
 * request as its lead (`lead`: the overview of the pull request and its changes are one document), the read of the plan, What Changes one header per file, the
 * contents of the steps (each its full title, the lead of its body, its facts and the Look closely of its author),
 * Possible Breakage with what breaks nothing and why the uses are unknown folded under it, and what changed since the
 * review. Rust decides and ranks; this words what it sent, in its order.
 */
export function ReviewPrologue({
  state,
  commit = null,
  headline = true,
  lead = null,
}: {
  state: ReviewPlanState | null
  commit?: PrologueCommit | null
  headline?: boolean
  /** What the author of a pull request wrote, its body, the lead of the document before its sections */
  lead?: ReactNode
}) {
  const plan = state?.plan ?? null
  const view = plan === null ? null : presentPrologue(plan, state?.pending === true)
  // the story of the commit shown alone, told before the steps of its concerns: in its heading, in place of its message
  // (the plan names a commit short, the range in full)
  const untold = view?.untold != null && commit !== null && state?.current === true && commit.commit.id.startsWith(view.untold.story.commit) ? view.untold : null
  return (
    // one reading column in the middle of the island: every block ends at its edge
    <ReadingColumn as="section" aria-label="Overview" data-testid="review-overview" className="gap-6 px-1 pt-3 pb-1">
      {commit !== null ? <CommitPart commit={commit} story={untold?.story ?? null} /> : view !== null && headline && <Headline issue={view.issue} title={view.title} />}
      {lead !== null && (
        <div className="flex min-w-0 flex-col" data-testid="review-overview-lead">
          {lead}
        </div>
      )}
      {(state?.loading === true || state?.pending === true) && (
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground" data-testid="review-overview-reading">
          <Spinner className="size-3" />
          {plan === null ? "Reading the review plan…" : "Finding the uses left outside…"}
        </span>
      )}
      {state?.error != null && (
        <p className="text-destructive" data-testid="review-plan-error">
          {`Cannot read the review plan: ${state.error}`}
        </p>
      )}
      {/* a commit shown alone shows its message in its heading */}
      {view !== null && commit === null && view.untold !== null && (
        <div className="flex flex-col gap-1.5" data-testid="review-overview-untold">
          <span className="font-medium">{view.untold.subject}</span>
          <StoryBody story={view.untold.story} />
        </div>
      )}
      {view !== null && <PrologueSections view={view} />}
    </ReadingColumn>
  )
}

/**
 * The title of the issue the commits name once YouTrack told it, else the name Rust gives the review (the branch as the
 * title bar names it, the range); the issue key a chip after it, a link with the popover of its issue
 */
function Headline({ issue, title }: { issue: string | null; title: string }) {
  const key = issue === null ? null : getYouTrackKeyOfIssue(issue)
  const entry = useIssueEntry(key ?? "")
  useEffect(() => {
    if (key !== null) {
      void loadIssue(key)
    }
  }, [key])
  const summary = key !== null && entry?.status === "found" ? entry.issue.summary : null
  return (
    <div className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-1">
      <h2 className="min-w-0 text-lg font-semibold [overflow-wrap:anywhere]" data-testid="review-overview-headline">
        {summary ?? title}
      </h2>
      {issue !== null && (
        <Badge variant="secondary" className="font-mono" data-testid="review-overview-issue">
          <LinkedText text={issue} linkClassName="text-foreground underline-offset-2 hover:underline" />
        </Badge>
      )}
    </div>
  )
}

/**
 * The commit shown alone: its heading, its whole message (of the concerns of its plan, the story told before them),
 * its actions; a commit with a body has the mark of its card after its subject, as its row has anywhere else: its
 * hover, the hover of the subject and `I` on the focused heading open the card (`CommitRowMark`), Escape gives the
 * focus back to the heading. A review of one commit lists it in no row, so its heading is its row
 */
function CommitPart({ commit: { commit, action, menu }, story }: { commit: PrologueCommit; story: StoryView | null }) {
  const heading = useRef<HTMLDivElement>(null)
  // kept across a move of the overview into the diffs as they load (`HeadingCardContext`), with the focus it held
  const [card, setCard] = useHeadingCard(commit.id, heading)
  const state: RowCardState = {
    open: card.open,
    openedByKey: card.open && card.byKey,
    onOpenChange: open => {
      if (open !== card.open) {
        setCard(open, false)
      }
    },
    focusRow: () => heading.current?.focus(),
  }
  const hover = useRowCardHover(state)
  const cardCommit = toCardCommit(commit)
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    // the physical key, in any keyboard layout: the card of the commit, the focus in it
    if (event.target === event.currentTarget && event.code === "KeyI" && commit.hasBody && !event.altKey && !event.metaKey && !event.ctrlKey) {
      event.preventDefault()
      setCard(true, true)
    }
  }
  return (
    <div
      ref={heading}
      role="group"
      aria-label={commit.subject}
      tabIndex={commit.hasBody ? 0 : undefined}
      // the ring of the focus a little off the text
      className="-m-1.5 flex min-w-0 flex-col rounded-md p-1.5 outline-none focus-visible:bd-focus"
      data-testid="range-review-commit-header"
      onKeyDown={handleKeyDown}
    >
      <CommitCard
        commit={cardCommit}
        variant="document"
        addon={
          <>
            {commit.hasBody && (
              // on the middle of the first line of the subject, which is taller than a row's
              <span className="flex shrink-0 pt-1">
                <CommitRowMark commit={cardCommit} menu={menu} popupHover={hover.popup} {...state} />
              </span>
            )}
            {action}
          </>
        }
        subjectProps={commit.hasBody ? hover.text : undefined}
        menu={menu}
        body={
          story === null ? undefined : (
            <>
              {story.subjectBefore !== undefined && <SubjectBefore subject={story.subjectBefore} />}
              <StoryBody story={story} />
            </>
          )
        }
      />
    </div>
  )
}

/** What Changes, the steps, Possible Breakage and what changed since the review */
function PrologueSections({ view }: { view: PrologueView }) {
  const navigator = useSelectReviewFile()
  const { breakage, since } = view
  return (
    <>
      {view.contract.length > 0 && (
        <div className="flex flex-col gap-2" data-testid="review-brief-contract">
          <SectionLabel title="What Changes" />
          <ul className="flex flex-col gap-5">
            {view.contract.map(file => (
              <li key={file.key} className="flex min-w-0 flex-col gap-3" data-testid="review-brief-contract-entry" data-path={file.path}>
                <span className="flex min-w-0 items-center gap-2 text-xs">
                  <Badge variant="secondary">{file.kind}</Badge>
                  <FileButton place={fileTop(file.path)} title={file.path} testId="review-brief-contract-file">
                    <span className="text-muted-foreground">{file.path}</span>
                  </FileButton>
                </span>
                {file.entries.map(entry => (
                  // under the chip of the kind, at its left edge: the heading of the section once, its tests, then its
                  // paragraphs whole, half a line apart
                  <div key={entry.key} className="flex min-w-0 flex-col gap-2" data-testid="review-brief-section">
                    {(entry.heading !== null || entry.provenBy !== null) && (
                      <div className="flex min-w-0 flex-wrap items-baseline gap-x-2">
                        {entry.heading !== null && <h3 className="min-w-0 font-semibold">{entry.heading}</h3>}
                        {entry.provenBy !== null && <span className="text-xs text-muted-foreground">{entry.provenBy}</span>}
                      </div>
                    )}
                    {entry.paragraphs.map(paragraph => (
                      <ContractParagraphView key={paragraph.key} paragraph={paragraph} />
                    ))}
                  </div>
                ))}
              </li>
            ))}
          </ul>
        </div>
      )}
      {view.steps.length > 0 && (
        <div className="flex flex-col gap-2" data-testid="review-contents">
          <div className="flex flex-col gap-0.5">
            <SectionLabel title="Steps" count={view.steps.length} />
            {/* why the steps are tasks, right under their count, not at the far end of the column */}
            {view.source !== null && (
              <p className="text-xs text-muted-foreground" data-testid="review-contents-source">
                {view.source}
              </p>
            )}
          </div>
          <ol className="flex flex-col gap-3">
            {view.steps.map(step => (
              <ContentsItem key={step.id} step={step} onShow={navigator === null ? null : () => navigator.showStep(step.id)} />
            ))}
          </ol>
        </div>
      )}
      {view.facts !== null && <StepFactsLine facts={view.facts} testId="review-overview-facts" />}
      {view.indexNote !== null && !view.indexNote.underBreakage && (
        <p className="text-xs text-muted-foreground" data-testid="review-plan-index-note">
          {view.indexNote.text}
        </p>
      )}
      {(breakage.entries.length > 0 || breakage.unbroken !== null || view.indexNote?.underBreakage === true) && (
        <div className="flex flex-col gap-1.5" data-testid="review-plan-breakage">
          {breakage.entries.length > 0 && (
            <>
              <SectionLabel title="Possible Breakage" count={breakage.entries.length} className="text-warning" />
              <ul aria-label="Possible breakage" className="flex flex-col gap-1.5">
                {breakage.entries.map(entry => (
                  <BreakageItem key={entry.key} entry={entry} />
                ))}
              </ul>
            </>
          )}
          {breakage.unbroken !== null && (
            <Disclosure tone="muted" title={breakage.unbroken.label} testId="review-plan-unbroken" className="text-xs font-medium">
              <ul aria-label="API changes no use left outside breaks" className="flex flex-col gap-0.5">
                {breakage.unbroken.entries.map(entry => (
                  <li key={entry.key} className="flex min-w-0 items-center gap-2" data-testid="review-plan-unbroken-entry" data-name={entry.name}>
                    <Badge variant={entry.tone}>{entry.kind}</Badge>
                    <FileButton place={entry.place} title={entry.location}>
                      <code className="font-mono">{entry.name}</code>
                      <span className="text-xs text-muted-foreground">{entry.location}</span>
                    </FileButton>
                  </li>
                ))}
              </ul>
            </Disclosure>
          )}
          {view.indexNote?.underBreakage === true && (
            <Disclosure tone="muted" title="Why the uses are unknown" testId="review-plan-index-note" className="text-xs font-medium">
              <p className="font-normal text-muted-foreground">{view.indexNote.text}</p>
            </Disclosure>
          )}
        </div>
      )}
      {since !== null && (
        <div className="flex flex-col gap-0.5" data-testid="review-plan-since">
          <span>{since.summary}</span>
          {since.rewritten.map(commit =>
            commit.message === null ? (
              <span key={commit.key} className="text-xs text-muted-foreground" title={commit.title}>
                {commit.text}
              </span>
            ) : (
              // the line opens to what changed in the message: no opening tells a commit of Since Your Review as one diff
              <Disclosure key={commit.key} tone="muted" title={commit.text} testId="review-plan-since-commit" className="text-xs">
                <MessageChangeView change={commit.message} />
              </Disclosure>
            ),
          )}
        </div>
      )}
    </>
  )
}

/**
 * A step of the contents: its number hanging in the gutter, so its full title, which shows its opening, and the lead of
 * its body stand at the edge of the column; its facts; the Look closely of its author, the one view of that part of a
 * body (`StoryPart`)
 */
function ContentsItem({ step, onShow }: { step: ContentsStep; onShow: (() => void) | null }) {
  return (
    <li className="flex min-w-0 flex-col gap-1" data-testid="review-contents-step" data-step={step.id}>
      <span className="-ml-6 flex min-w-0 items-baseline gap-2">
        <span className="w-4 shrink-0 text-right text-muted-foreground tabular-nums">{step.number}</span>
        <button
          type="button"
          className={cn("min-w-0 rounded-sm text-left font-medium outline-none hover:underline focus-visible:bd-focus disabled:no-underline", step.code && "font-mono")}
          disabled={onShow === null}
          data-testid="review-contents-step-title"
          onClick={() => onShow?.()}
        >
          {step.title}
        </button>
      </span>
      <div className="flex min-w-0 flex-col gap-1">
        {step.lead !== null && (
          <div data-testid="review-contents-step-lead">
            <StoryBlock story={step.lead.story} placed={step.lead.block} told={false} />
          </div>
        )}
        {step.facts.unproven !== null && (
          <span className="text-xs text-warning" data-testid="review-contents-step-unproven">
            {step.facts.unproven}
          </span>
        )}
        <StepFactsLine facts={step.facts} testId="review-contents-step-facts" />
        {step.lookClosely.map(({ story, part }) => (
          <div key={part.key} data-testid="review-contents-step-look-closely">
            <StoryPart story={story} part={part} />
          </div>
        ))}
      </div>
    </li>
  )
}

/**
 * A removed or changed API: its kind and name show its declaration; its forms before and after, then the uses left
 * outside, each opening its file at the head of the review
 */
function BreakageItem({ entry }: { entry: BreakageEntry }) {
  const navigator = useSelectReviewFile()
  const titles = [...entry.sites.map(site => site.label), ...(entry.moreSites === null ? [] : [entry.moreSites])]
  return (
    <li className="flex min-w-0 flex-col" data-testid="review-plan-breakage-entry" data-name={entry.name} data-path={entry.path}>
      <div className="flex min-w-0 items-center gap-2">
        <Badge variant={entry.tone}>{entry.kind}</Badge>
        <FileButton place={entry.place} title={entry.location} testId="review-plan-breakage-file">
          <code className="font-mono">{entry.name}</code>
          <span className="text-xs text-muted-foreground">{entry.location}</span>
        </FileButton>
        {entry.changed !== null && <span className="shrink-0 text-xs text-muted-foreground">{entry.changed}</span>}
        <span className="shrink-0 text-xs text-warning tabular-nums" data-testid="review-plan-outside">
          {entry.outside}
        </span>
      </div>
      {entry.before !== null && (
        <code className="truncate pl-3 font-mono text-xs text-muted-foreground" title={entry.before} data-testid="review-plan-before">
          {`− ${entry.before}`}
        </code>
      )}
      {entry.after !== null && (
        <code className="truncate pl-3 font-mono text-xs text-muted-foreground" title={entry.after} data-testid="review-plan-after">
          {`+ ${entry.after}`}
        </code>
      )}
      {(titles.length > 0 || entry.inside !== null || entry.stillBound !== null || entry.byName !== null) && (
        <span className="flex min-w-0 flex-wrap items-baseline gap-x-1.5 pl-3 text-xs text-muted-foreground" title={titles.join("\n")} data-testid="review-plan-sites">
          {[
            ...entry.sites.map(site => (
              <button
                key={site.label}
                type="button"
                className="rounded-sm outline-none hover:underline focus-visible:bd-focus disabled:no-underline"
                disabled={navigator === null}
                data-testid="review-plan-site"
                data-path={site.place.path}
                onClick={() => navigator?.showOutside(site.place)}
              >
                {site.label}
              </button>
            )),
            ...[entry.moreSites, entry.inside, entry.stillBound, entry.byName].filter(part => part !== null).map(part => <span key={part}>{part}</span>),
          ].map((part, index) => (
            // the parts apart as the line of a list: "src/Base.kt:1 · src/Uses.kt:4 · 1 use updated in this change"
            <span key={part.key} className="flex items-baseline gap-x-1.5">
              {index > 0 && <span aria-hidden="true">·</span>}
              {part}
            </span>
          ))}
        </span>
      )}
    </li>
  )
}

/**
 * What a click shows in the review: a changed file, by its path or its old path, at the line of the entry; plain text
 * outside a review
 */
function FileButton({ place, title, testId = "review-plan-file", children }: { place: ReviewPlace; title: string; testId?: string; children: ReactNode }) {
  const navigator = useSelectReviewFile()
  return (
    <button
      type="button"
      className="flex min-w-0 items-baseline gap-2 truncate rounded-sm text-left outline-none hover:underline focus-visible:bd-focus disabled:no-underline"
      title={title}
      disabled={navigator === null}
      data-testid={testId}
      data-path={place.path}
      onClick={() => navigator?.show(place)}
    >
      {children}
    </button>
  )
}

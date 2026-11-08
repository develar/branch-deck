import { useEffect, useRef } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CommitCard } from "@/components/commit/CommitCard"
import { CommitRowMark } from "@/components/commit/CommitRowMark"
import { LinkedText } from "@/components/commit/LinkedText"
import { ContractParagraphView, ContractTableView } from "@/components/review/ContractParagraphView"
import { MessageChangeView } from "@/components/review/MessageChangeView"
import { StepFactsLine, UnprovenFact } from "@/components/review/StepFactsLine"
import { StoryBlock, StoryBody, StoryPart, SubjectBefore } from "@/components/review/StoryBody"
import { BreakableCode } from "@/components/shared/CodeText"
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
          <SectionLabel title="What Changes" className="text-sm font-semibold text-foreground" />
          {view.substitutions.map(substitution => (
            <div key={substitution.key} className="flex flex-wrap items-baseline gap-x-1.5 gap-y-1 text-sm" data-testid="review-brief-substitution">
              <span>
                Renames <code>{substitution.from}</code> → <code>{substitution.to}</code> · {substitution.summary}
              </span>
              {substitution.files.length > 0 && (
                <span className="flex flex-wrap items-baseline gap-x-1.5">
                  <span aria-hidden="true">·</span>
                  {substitution.files.slice(0, 3).map(file =>
                    file.place === null ? (
                      <span key={file.path} className="text-muted-foreground">
                        {file.name}
                      </span>
                    ) : (
                      <FileButton key={file.path} place={file.place} title={file.path} testId="review-brief-substitution-file">
                        <span className="text-foreground">{file.name}</span>
                      </FileButton>
                    ),
                  )}
                  {substitution.files.length > 3 && (
                    <Tooltip>
                      <TooltipTrigger
                        render={
                          <button
                            type="button"
                            className="rounded-sm px-1 text-muted-foreground underline decoration-dotted underline-offset-2 outline-none hover:bg-muted hover:text-foreground focus-visible:bd-focus"
                            aria-label={`Show ${substitution.files.length - 3} more files`}
                            data-testid="review-brief-substitution-more"
                          />
                        }
                      >
                        and {substitution.files.length - 3} more
                      </TooltipTrigger>
                      <TooltipContent className="max-w-sm whitespace-pre-line">{substitution.allFiles}</TooltipContent>
                    </Tooltip>
                  )}
                </span>
              )}
            </div>
          ))}
          <ul className="flex flex-col gap-5">
            {view.contract.map(file => (
              <li key={file.key} className="flex min-w-0 flex-col gap-3" data-testid="review-brief-contract-entry" data-path={file.path}>
                <span className="flex min-w-0 items-center gap-2 text-sm">
                  <Badge variant="secondary">{file.kind}</Badge>
                  <FileButton place={fileTop(file.path)} title={file.path} testId="review-brief-contract-file">
                    <span className="min-w-0">
                      {file.path.includes("/") && (
                        <span className="text-muted-foreground">
                          <BreakableCode text={file.path.slice(0, file.path.lastIndexOf("/") + 1)} />
                        </span>
                      )}
                      <span className="font-semibold text-foreground">
                        <BreakableCode text={file.path.slice(file.path.lastIndexOf("/") + 1)} />
                      </span>
                    </span>
                  </FileButton>
                </span>
                <div className="flex min-w-0 flex-col gap-3 border-l-2 border-border pl-3">
                  {file.entries.map(entry => (
                    // the heading of the section once, its tests, then its paragraphs whole and its tables, half a line apart
                    <div key={entry.key} className="flex min-w-0 flex-col gap-2" data-testid="review-brief-section">
                      {(entry.heading !== null || entry.provenBy !== null) && (
                        <div className="flex min-w-0 flex-wrap items-baseline gap-x-2">
                          {entry.heading !== null && <h3 className="min-w-0 text-sm font-semibold">{entry.heading}</h3>}
                          {entry.provenBy !== null && <span className="text-xs text-muted-foreground">{entry.provenBy}</span>}
                        </div>
                      )}
                      {entry.blocks.map(block =>
                        block.kind === "table" ? (
                          <ContractTableView key={block.key} table={block} />
                        ) : (
                          <ContractParagraphView key={block.paragraph.key} paragraph={block.paragraph} />
                        ),
                      )}
                    </div>
                  ))}
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
      {view.steps.length > 0 && (
        <div className="flex flex-col gap-2" data-testid="review-contents">
          <div className="flex flex-col gap-0.5">
            <SectionLabel title="Steps" count={view.steps.filter(step => step.number !== null).length} className="text-sm font-semibold text-foreground" />
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
      {view.facts !== null && <StepFactsLine facts={view.facts} testId="review-overview-facts" lines />}
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
                    <Badge variant={entry.tone} className="shrink-0">
                      {entry.kind}
                    </Badge>
                    <FileButton place={entry.place} title={entry.location}>
                      <code className="shrink-0 font-mono">{entry.name}</code>
                      <span className="min-w-0 truncate text-xs text-muted-foreground" dir="rtl">
                        <bdi dir="ltr">{entry.location}</bdi>
                      </span>
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
 * A step of the contents: its number, when any, hanging in the gutter, so its full title, which shows its opening, and
 * the lead of its body stand at the edge of the column; its facts; the Look closely of its author, the one view of that
 * part of a body (`StoryPart`)
 */
function ContentsItem({ step, onShow }: { step: ContentsStep; onShow: (() => void) | null }) {
  return (
    <li className="flex min-w-0 flex-col gap-1" data-testid="review-contents-step" data-step={step.id}>
      <span className={cn(step.number !== null && "-ml-6", "flex min-w-0 items-baseline gap-2")}>
        {step.number !== null && (
          <span className="w-4 shrink-0 text-right text-muted-foreground tabular-nums" data-testid="review-contents-step-number">
            {step.number}
          </span>
        )}
        <button
          type="button"
          className={cn("min-w-0 rounded-sm text-left font-medium outline-none hover:underline focus-visible:bd-focus disabled:no-underline", step.code && "font-mono")}
          disabled={onShow === null}
          data-testid="review-contents-step-title"
          onClick={() => onShow?.()}
        >
          {step.code ? <BreakableCode text={step.title} humps /> : step.title}
        </button>
      </span>
      <div className="flex min-w-0 flex-col gap-1">
        {step.lead !== null && (
          <div data-testid="review-contents-step-lead">
            <StoryBlock story={step.lead.story} placed={step.lead.block} told={false} />
          </div>
        )}
        {step.facts.unproven !== null && <UnprovenFact unproven={step.facts.unproven} testId="review-contents-step-unproven" />}
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
 * A removed or changed API: its kind and its whole name, which shows its declaration; under it its file and line, the
 * parts that changed and the uses left outside; its forms before and after, wrapping, a line per constructor; then the
 * uses left outside, a line each (its file name and line, its directory muted, opening its file at the head of the
 * review), and the counts of the rest. Every line ends at the edge of the column
 */
function BreakageItem({ entry }: { entry: BreakageEntry }) {
  const navigator = useSelectReviewFile()
  const counts = [entry.moreSites, entry.inside, entry.stillBound, entry.byName].filter(part => part !== null)
  return (
    <li className="flex min-w-0 flex-col gap-0.5" data-testid="review-plan-breakage-entry" data-name={entry.name} data-path={entry.path}>
      <div className="flex min-w-0 items-baseline gap-2">
        <Badge variant={entry.tone} className="shrink-0">
          {entry.kind}
        </Badge>
        <FileButton place={entry.place} title={entry.location} testId="review-plan-breakage-file">
          <code className="font-mono [overflow-wrap:anywhere]">
            <BreakableCode text={entry.name} humps />
          </code>
        </FileButton>
      </div>
      <div className="flex min-w-0 flex-wrap items-baseline gap-x-1.5 text-xs text-muted-foreground">
        <span className="min-w-0 truncate" title={entry.location} dir="rtl">
          <bdi dir="ltr">{entry.location}</bdi>
        </span>
        {entry.changed !== null && (
          <>
            <span aria-hidden="true">·</span>
            <span className="shrink-0">{entry.changed}</span>
          </>
        )}
        <span aria-hidden="true">·</span>
        <span className="shrink-0 text-warning tabular-nums" data-testid="review-plan-outside">
          {entry.outside}
        </span>
      </div>
      {entry.before !== null && <EntryForm sign="−" form={entry.before} testId="review-plan-before" />}
      {entry.after !== null && <EntryForm sign="+" form={entry.after} testId="review-plan-after" />}
      {(entry.sites.length > 0 || counts.length > 0) && (
        <div className="flex min-w-0 flex-col pl-3 text-xs text-muted-foreground" data-testid="review-plan-sites">
          {entry.sites.map(site => (
            <button
              key={site.label}
              type="button"
              className="flex max-w-full min-w-0 items-baseline gap-1.5 self-start rounded-sm text-left outline-none hover:underline focus-visible:bd-focus disabled:no-underline"
              title={site.label}
              disabled={navigator === null}
              data-testid="review-plan-site"
              data-path={site.place.path}
              onClick={() => navigator?.showOutside(site.place)}
            >
              <span className="shrink-0 text-foreground">{site.name}</span>
              {site.directory !== "" && (
                <span className="min-w-0 truncate" dir="rtl">
                  <bdi dir="ltr">{site.directory}</bdi>
                </span>
              )}
            </button>
          ))}
          {counts.length > 0 && (
            // the counts of the rest apart as the line of a list: "+4 more · 2 uses updated in this change"
            <span className="flex min-w-0 flex-wrap items-baseline gap-x-1.5">
              {counts.map((part, index) => (
                <span key={part} className="flex items-baseline gap-x-1.5">
                  {index > 0 && <span aria-hidden="true">·</span>}
                  {part}
                </span>
              ))}
            </span>
          )}
        </div>
      )}
    </li>
  )
}

/** A form of an entry before (−) or after (+): code that wraps at its spaces, a line per constructor, the sign hanging */
function EntryForm({ sign, form, testId }: { sign: string; form: string; testId: string }) {
  return (
    <code className="flex min-w-0 gap-1.5 pl-3 font-mono text-xs text-muted-foreground" data-testid={testId}>
      <span aria-hidden="true" className="shrink-0">
        {sign}
      </span>
      <span className="min-w-0 [overflow-wrap:anywhere] whitespace-pre-line">{form}</span>
    </code>
  )
}

/**
 * What a click shows in the review: a changed file, by its path or its old path, at the line of the entry; plain text
 * outside a review
 */
function FileButton({ place, title, testId = "review-plan-file", children }: { place: ReviewPlace; title: string; testId?: string; children: ReactNode }) {
  const navigator = useSelectReviewFile()
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            className="flex min-w-0 items-baseline gap-2 rounded-sm text-left outline-none hover:underline focus-visible:bd-focus disabled:no-underline"
            disabled={navigator === null}
            data-testid={testId}
            data-path={place.path}
            onClick={() => navigator?.show(place)}
          />
        }
      >
        {children}
      </TooltipTrigger>
      <TooltipContent>{title}</TooltipContent>
    </Tooltip>
  )
}

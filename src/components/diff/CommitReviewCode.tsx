import { useImperativeHandle, useRef, useState } from "react"
import type { ReactNode, Ref } from "react"
import { cn } from "cn"
import { BookOpenTextIcon, UnfoldVerticalIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { DeclarationFileView } from "@/components/diff/DeclarationFileView"
import { DiffView } from "@/components/diff/DiffView"
import type { DeclarationPlace, DiffHeaderProps, DiffSection, DiffViewFile, DiffViewHandle } from "@/components/diff/DiffView"
import { SymbolChooser } from "@/components/diff/SymbolChooser"
import { SymbolSearchStatus } from "@/components/diff/SymbolSearchStatus"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FileHeaderChevron, FileHeaderCopyButton } from "@/components/files/FileHeaderParts"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import { ChapterOpening } from "@/components/review/ChapterOpening"
import { StoryText } from "@/components/review/StoryText"
import { HoverDescription } from "@/components/shared/HoverDescription"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import { placeOf, useSymbolNavigation } from "@/hooks/useSymbolNavigation"
import type { DeclarationTarget, DiffRevisions } from "@/lib/bindings"
import type { CommitFile } from "@/lib/bindings"
import { formatCommitFilePath } from "@/lib/commit-files"
import type { LayoutPart } from "@/lib/diff/layout"
import { presentCardEnd } from "@/presenters/review-card-end"
import type { OpeningView, StoryMark } from "@/presenters/review-story"
import { useDiffSoftWraps } from "@/stores/settings/diff-view-settings"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/** The files whose headers show while the diffs of a review are read */
const PENDING_HEADERS = 12

export interface CommitReviewCodeHandle extends DiffViewHandle {
  /** Opens a file outside the review over it, at the place of `target` */
  openOutside: (target: DeclarationTarget) => void
}

/**
 * A stop J shows after another (`CommitReviewCodeProps.nextSection`): the key of its section, its file and its part,
 * the opening of a step, the card, or the prologue above the first section; of an opening the step whose opening it is
 * (`DiffSection.before`: a section may hold the openings of several)
 */
export interface NextSection {
  readonly key: string
  readonly path: string
  readonly part: LayoutPart
  readonly step?: string
}

export interface CommitReviewCodeProps {
  ref?: Ref<CommitReviewCodeHandle>
  /** The files in the order of the tree, with their diffs; the texts are read as they come into view */
  files: readonly DiffViewFile[]
  diffViewMode: DiffViewMode
  /** The file shown first, at the top */
  initialPath: string | null
  /** The section whose header is at the top, its key, its file and its part at the top, as the diffs scroll */
  onVisibleSectionChange: (key: string, path: string, part: LayoutPart, step?: string) => void
  /** The first screen is painted: the window can show */
  onFirstScreen?: () => void
  /** The code owner of a file by its path, in its header */
  fileOwners?: ReadonlyMap<string, string>
  /** The commits of the sides: a declaration outside the review is read at them */
  revisions: DiffRevisions
  /** The files an owner chip marks: the headers of the others are muted; none muted without */
  marked?: ReadonlySet<string> | null
  /** A click on the title of the opening of a step: the opening at the top, as the bar of the step above the diffs */
  onShowStep?: (step: string) => void
  /** The overview before the first section: always mounted, measured in place */
  prologue?: ReactNode
  /** The opening of each step by its id, above the card of its first section */
  openings?: ReadonlyMap<string, OpeningView>
  /**
   * The steps of no card told after the last card, in their order: their openings are the epilogue of the diffs, each an
   * element `data-opening-step` of it
   */
  after?: readonly NonNullable<DiffSection["step"]>[]
  /**
   * What J shows after the section of a key, undefined when nothing is left: the line at the end of a long card and of
   * the last one names it; no such line without
   */
  nextSection?: (key: string) => NextSection | undefined
  /** A click on the line at the end of a card: the section it names, shown as J shows it */
  onShowSection?: (next: NextSection) => void
  /** The name of the chosen owner chip, whose files J steps through: the end of them is "No more files of Design" */
  ownerLabel?: string | null
}

/**
 * The diffs of all files of a commit or a range in one `DiffView`: a sticky header per file, the unchanged lines folded
 * as IntelliJ, lock and generated files, files over 1,000 changed lines and the files the review plan folds collapsed;
 * with the plan, the overview before the first section, the opening of each step above the card of its first section
 * (of a step of no card after the last card, below that card) and a divider above the first of each group; a long card
 * and the last one end with a line naming what J shows next.
 */
export function CommitReviewCode({
  ref,
  files,
  diffViewMode,
  initialPath,
  onVisibleSectionChange,
  onFirstScreen,
  fileOwners,
  revisions,
  marked = null,
  onShowStep,
  prologue,
  openings,
  after,
  nextSection,
  onShowSection,
  ownerLabel = null,
}: CommitReviewCodeProps) {
  const softWraps = useDiffSoftWraps()
  const review = useRef<DiffViewHandle>(null)
  // the files outside the review a jump opened, each with the name it started from, the last one shown over the review;
  // Back closes them one by one, the last one back to its name in the review
  const [fileViews, setFileViews] = useState<readonly { target: DeclarationTarget; from: DeclarationPlace | null }[]>([])
  // a file outside the review opens over it; a declaration of the review, reached from such a file, closes them all
  const openFile = (target: DeclarationTarget, from: DeclarationPlace | null) => {
    if (target.outside !== null) {
      setFileViews(views => [...views, { target, from }])
    } else {
      setFileViews([])
      review.current?.revealDeclaration(placeOf(target), null)
    }
  }
  // the tree and the keys of the review move in the review: they close the files over it
  useImperativeHandle(ref, () => ({
    scrollToFile: path => {
      setFileViews([])
      review.current?.scrollToFile(path)
    },
    scrollToSection: (key, part, step) => {
      setFileViews([])
      review.current?.scrollToSection(key, part, step)
    },
    moveChange: (direction, accept) => {
      setFileViews([])
      return review.current?.moveChange(direction, accept)
    },
    scrollToPrologue: () => {
      setFileViews([])
      review.current?.scrollToPrologue()
    },
    visibleStop: () => review.current?.visibleStop(),
    focus: () => review.current?.focus(),
    openFind: () => review.current?.openFind(),
    stepFind: direction => review.current?.stepFind(direction),
    revealDeclaration: (place, from) => {
      setFileViews([])
      return review.current?.revealDeclaration(place, from) ?? false
    },
    openOutside: target => openFile(target, null),
  }))
  const inReview = useSymbolNavigation(files, revisions, openFile)
  const shownFile = fileViews.at(-1)?.target
  const back = () => {
    const from = fileViews.length === 1 ? fileViews[0]!.from : null
    setFileViews(fileViews.slice(0, -1))
    if (from !== null) {
      review.current?.revealDeclaration(from, null)
    }
  }
  const inFile = useSymbolNavigation(files, { old: null, new: shownFile?.outside?.revision ?? null }, openFile)
  // the search in flight of either view, at its name
  const search = inReview.search ?? inFile.search
  const lastKey = files.length === 0 ? undefined : keyOf(files[files.length - 1]!)
  // a click on the line at the end of a card: the diffs keep the focus, as a fold button keeps it, so J goes on from
  // there; a focused line would unmount with its card once the review scrolls far from it, and the keys with it
  const showFromCardEnd = (next: NextSection) => {
    review.current?.focus()
    onShowSection?.(next)
  }
  return (
    <div className="relative h-full">
      <DiffView
        ref={review}
        files={files}
        mode={diffViewMode}
        softWraps={softWraps}
        initialPath={initialPath}
        label="Changes of the commit"
        onVisibleSectionChange={onVisibleSectionChange}
        onFirstScreen={onFirstScreen}
        onGoToDeclaration={inReview.goToDeclaration}
        onFindUsages={inReview.findUsages}
        renderHeader={header => <ReviewFileHeader {...header} owner={fileOwners?.get(header.file.path)} muted={marked !== null && !marked.has(header.file.path)} />}
        prologue={prologue}
        epilogue={after === undefined || after.length === 0 ? undefined : <LayerOfOpenings steps={after} openings={openings} group={null} onShowStep={onShowStep} />}
        renderOpening={section => <SectionOpenings section={section} openings={openings} onShowStep={onShowStep} />}
        renderDivider={section => <ReviewDivider section={section} />}
        muted={marked === null ? undefined : path => !marked.has(path)}
        renderFooter={
          nextSection === undefined
            ? undefined
            : (key, path) => {
                const next = nextSection(key)
                // the opening of a step of no card below the last card, by its words
                const ending = next?.part === "opening" ? after?.find(step => step.id === next.step) : undefined
                return (
                  <ReviewCardEnd
                    path={path}
                    next={next}
                    owner={key === lastKey ? null : ownerLabel}
                    step={ending === undefined ? null : `${ending.number} · ${ending.title}`}
                    onShow={showFromCardEnd}
                  />
                )
              }
        }
      />
      {shownFile !== undefined && (
        <DeclarationFileView
          key={`${fileViews.length}:${shownFile.path}:${shownFile.line}`}
          target={shownFile}
          onBack={back}
          onGoToDeclaration={inFile.goToDeclaration}
          onFindUsages={inFile.findUsages}
        />
      )}
      {search !== null && (
        <SymbolSearchStatus
          search={search}
          onCancel={() => {
            inReview.cancelSearch()
            inFile.cancelSearch()
          }}
        />
      )}
      <SymbolChooser
        choice={inReview.choice ?? inFile.choice}
        onClose={() => {
          inReview.closeChoice()
          inFile.closeChoice()
        }}
      />
    </div>
  )
}

/**
 * The headers of the first files while their patch loads: where the diffs will be, in the order of the tree, so the
 * window that shows before a slow git has its files on screen
 */
export function PendingFileHeaders({ files, fileOwners }: { files: readonly CommitFile[]; fileOwners?: ReadonlyMap<string, string> }) {
  return (
    // the cards the files will have, header only (diff-view.css)
    <div data-testid="review-pending-files" aria-busy="true" className="dv-cards flex flex-col gap-2.5 px-3 py-2.5">
      {files.slice(0, PENDING_HEADERS).map(file => (
        <div key={file.path} className="dv-card">
          <div className="dv-header">
            <ReviewFileHeader
              file={file}
              section={null}
              notice={null}
              empty
              folded={null}
              collapsed={false}
              onToggle={() => {}}
              pending
              owner={fileOwners?.get(file.path)}
              muted={false}
            />
          </div>
        </div>
      ))}
    </div>
  )
}

interface ReviewFileHeaderProps extends DiffHeaderProps {
  /** The diffs are not read yet: no notice, nothing to collapse */
  pending?: boolean
  owner: string | undefined
  /** Of another owner than the chosen chip's: played down, its card stays in the story */
  muted: boolean
}

/**
 * The openings above the card of a section: of the steps of no card told before its own (`DiffSection.before`), each
 * its element of the layer (`data-opening-step`), then of its own step with the group the section starts; each a card
 */
function SectionOpenings({ section, openings, onShowStep }: { section: DiffSection; openings: ReadonlyMap<string, OpeningView> | undefined; onShowStep?: (step: string) => void }) {
  return <LayerOfOpenings steps={[...(section.before ?? []), section.step!]} openings={openings} group={section.heading} onShowStep={onShowStep} />
}

/** The openings of several steps in one layer, each its element (`data-opening-step`), the last with `group` */
function LayerOfOpenings({
  steps,
  openings,
  group,
  onShowStep,
}: {
  steps: readonly NonNullable<DiffSection["step"]>[]
  openings: ReadonlyMap<string, OpeningView> | undefined
  group: string | null
  onShowStep?: (step: string) => void
}) {
  return (
    // each opening a card of prose, a gap above the card of the section (diff-view.css)
    <div className="dv-openings">
      {steps.map((step, index) => (
        <div key={step.id} data-opening-step={step.id} className="dv-prose dv-step-opening">
          <ChapterOpening opening={openingOf(step, openings)} group={index === steps.length - 1 ? group : null} onShowStep={onShowStep} />
        </div>
      ))}
    </div>
  )
}

/** The opening of a step: its words, else its number and title alone */
function openingOf(step: NonNullable<DiffSection["step"]>, openings: ReadonlyMap<string, OpeningView> | undefined): OpeningView {
  return (
    openings?.get(step.id) ?? {
      id: step.id,
      number: step.number,
      title: step.title,
      code: false,
      titleInBody: false,
      subjectBefore: null,
      evidence: [],
      story: null,
      commits: [],
      facts: {
        line: null,
        tests: [],
        proves: [],
        newApi: { places: [], more: 0 },
        moduleLinks: { places: [], more: 0 },
        takenOutBy: null,
        shownElsewhere: [],
        unproven: null,
      },
    }
  )
}

/**
 * The divider above the card of a section that starts a group of the story and no step (spec/branch/review-tour.spec.md,
 * "In the Review Window"): the group muted ("Same edit in 4 files"), its words at the bottom near the card they head
 */
function ReviewDivider({ section }: { section: DiffSection }) {
  return (
    <div className="flex h-full min-w-0 items-end px-1 pb-1.5">
      <span data-testid="review-section-heading" className="truncate text-xs text-muted-foreground">
        {section.heading}
      </span>
    </div>
  )
}

/** The key of the section of a file of the review (`DiffView`'s `PreparedFile.key`) */
function keyOf(file: DiffViewFile): string {
  return file.section?.key ?? file.file.path
}

/**
 * The line at the end of a long card and of the last one (spec/commit/commit-review.spec.md): what J shows next, its
 * name and its directory as its header names them, which a click shows as J does, or the end (`presentCardEnd`);
 * `owner` before the last card, the chosen owner chip whose files J steps through. Not focusable: J does it from the keys
 */
function ReviewCardEnd({
  path,
  next,
  owner,
  step,
  onShow,
}: {
  path: string
  next: NextSection | undefined
  owner: string | null
  /** The words of the step of no card after the last card whose opening is `next` */
  step: string | null
  onShow: (next: NextSection) => void
}) {
  const end = presentCardEnd(path, next?.path, owner, step)
  if (next === undefined || end.kind === "end") {
    return <span data-testid="review-card-end">{end.kind === "end" ? end.text : null}</span>
  }
  return (
    <button
      type="button"
      tabIndex={-1}
      data-testid="review-card-end"
      aria-label={end.label}
      title={next.path}
      className="flex min-w-0 cursor-pointer items-center gap-1.5 rounded-sm px-1.5 py-0.5 outline-none hover:bg-accent hover:text-accent-foreground"
      onClick={() => onShow(next)}
    >
      <span className="shrink-0">Next:</span>
      {end.step ? (
        <span className="min-w-0 truncate font-semibold text-foreground">{end.name}</span>
      ) : end.continued ? (
        <>
          <span className="min-w-0 truncate font-semibold text-foreground">{`${end.name},`}</span>
          <span className="shrink-0">continued</span>
        </>
      ) : (
        // the file as the header of its card names it: the name bold, the directory muted, its last folder not
        <FilePathLabel path={next.path} emphasis className="text-foreground" />
      )}
      <Kbd className="h-4 min-w-4">J</Kbd>
    </button>
  )
}

/**
 * The sticky header of a file, the top of its card: the chevron, the status letter, the name bold and the path muted,
 * as the tree row says, Expand All, its owner and counts, and the menu of the file; the step and the group of a section
 * are on its divider, so the name of every file stands at one x. The insets are IntelliJ's: 12 px above and below, 10
 * px at the sides
 */
function ReviewFileHeader({ file, section, notice, empty, folded, pending = false, owner, muted, collapsed, onToggle, onExpandAll }: ReviewFileHeaderProps) {
  const name = file.path.slice(file.path.lastIndexOf("/") + 1)
  return (
    <FileContextMenu
      target={() => ({ kind: "file", path: file.path })}
      selectable
      render={
        <div
          data-testid="review-file-header"
          data-path={file.path}
          data-muted={muted ? "" : undefined}
          // its parts fade, not the header: its background hides the rows that scroll under it
          className={cn("group/file-header flex items-center gap-2 px-2.5 text-sm", muted && "*:opacity-60")}
        />
      }
    >
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label={collapsed ? `Expand ${name}` : `Collapse ${name}`}
        aria-expanded={!collapsed}
        disabled={pending || empty}
        onClick={onToggle}
      >
        <FileHeaderChevron open={!collapsed} />
      </Button>
      <span className="flex min-w-0" title={formatCommitFilePath(file)}>
        <FilePathLabel path={file.path} oldPath={file.oldPath} status={file.status} emphasis />
      </span>
      <FileHeaderCopyButton path={file.path} />
      <span className="ml-auto flex shrink-0 items-center gap-3 text-xs">
        {section?.story !== undefined && section.story.length > 0 && <StoryMarkButton marks={section.story} name={name} />}
        {/* an action on the rows of the file, with the other parts at the end, named on hover as IntelliJ's */}
        {onExpandAll !== undefined && (
          <Tooltip>
            <TooltipTrigger render={<Button variant="ghost" size="icon-xs" aria-label={`Expand all unchanged lines of ${name}`} onClick={onExpandAll} />}>
              <UnfoldVerticalIcon />
            </TooltipTrigger>
            <TooltipContent>Expand All</TooltipContent>
          </Tooltip>
        )}
        {folded !== null && (
          <span data-testid="review-file-folded" className="text-muted-foreground">
            {folded}
          </span>
        )}
        {notice !== null &&
          (empty ? (
            <span className="text-muted-foreground">{notice}</span>
          ) : (
            <span data-testid="review-file-line-ends" className="text-muted-foreground">
              {notice}
            </span>
          ))}
        {owner !== undefined && (
          <span data-testid="review-file-owner" className="text-muted-foreground" title={`Owned by ${owner}`}>
            {owner}
          </span>
        )}
        {(file.additions > 0 || file.deletions > 0) && <LineCounts additions={file.additions} deletions={file.deletions} colored />}
      </span>
    </FileContextMenu>
  )
}

/**
 * The mark of a section a body names (spec/branch/review-tour.spec.md, "Anchors"), at the end of its header, which keeps
 * its height: its hover opens a card of the story that names the file, where it is told once ("Step 5 · title"), then
 * each line that names it, quoted as the story sets it, its code names links; a click shows the first of them in its
 * opening (or the overview), outlined
 */
function StoryMarkButton({ marks, name }: { marks: readonly StoryMark[]; name: string }) {
  const navigator = useSelectReviewFile()
  const first = marks[0]!
  // the marks of one place under one heading, in their order
  const places: { title: string; head: StoryMark; marks: StoryMark[] }[] = []
  for (const mark of marks) {
    const place = places.find(it => it.title === mark.title)
    if (place === undefined) {
      places.push({ title: mark.title, head: mark, marks: [mark] })
    } else {
      place.marks.push(mark)
    }
  }
  // the card opens on the hover of the mark and on its focus from the keyboard, as the description of a control does
  const [open, setOpen] = useState(false)
  const trigger = useRef<HTMLButtonElement>(null)
  return (
    <HoverDescription lazy open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={
          <Button
            ref={trigger}
            variant="ghost"
            size="icon-xs"
            data-testid="review-story-mark"
            aria-label={`The story that names ${name}: ${first.title}`}
            disabled={navigator === null}
            className="text-muted-foreground"
            onClick={() => navigator?.showStory(first)}
            onFocus={event => {
              if (event.currentTarget.matches(":focus-visible")) {
                setOpen(true)
              }
            }}
            // the card is read from the keyboard, not entered: Enter shows the story, whose names are links there
            onBlur={() => setOpen(false)}
          />
        }
      >
        <BookOpenTextIcon />
      </PopoverTrigger>
      <PopoverContent
        // above the header of its section, never over the lines of the section it tells of; below the whole header where
        // no room is above it, never over the header
        anchor={() => trigger.current?.closest<HTMLElement>("[data-testid=review-file-header]") ?? trigger.current}
        side="top"
        align="end"
        collisionPadding={8}
        className="w-auto max-w-md gap-3 p-3"
        data-testid="review-story-mark-tip"
        // a card of words: the focus stays where it is
        initialFocus={false}
        finalFocus={false}
      >
        {places.map(place => (
          <div key={place.title} className="flex min-w-0 flex-col gap-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              {place.head.place}
              {place.head.label !== null && (
                <>
                  {" · "}
                  <span className={cn(place.head.code && "font-mono")}>{place.head.label}</span>
                </>
              )}
            </span>
            {place.marks.map(mark => (
              <p key={mark.key} className="border-l-2 border-border pl-2.5 [font-feature-settings:'ss04'] text-sm">
                <StoryText text={mark.preview} spans={mark.spans} mentions={mark.mentions} nested />
              </p>
            ))}
          </div>
        ))}
      </PopoverContent>
    </HoverDescription>
  )
}

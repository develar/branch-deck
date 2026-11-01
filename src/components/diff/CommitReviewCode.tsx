import { useImperativeHandle, useRef, useState } from "react"
import type { Ref } from "react"
import { cn } from "cn"
import { UnfoldVerticalIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
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
import { placeOf, useSymbolNavigation } from "@/hooks/useSymbolNavigation"
import type { DeclarationTarget, DiffRevisions } from "@/lib/bindings"
import type { CommitFile } from "@/lib/bindings"
import { formatCommitFilePath } from "@/lib/commit-files"
import { presentCardEnd } from "@/presenters/review-card-end"
import { useDiffSoftWraps } from "@/stores/settings/diff-view-settings"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/** The files whose headers show while the diffs of a review are read */
const PENDING_HEADERS = 12

export interface CommitReviewCodeHandle extends DiffViewHandle {
  /** Opens a file outside the review over it, at the place of `target` */
  openOutside: (target: DeclarationTarget) => void
}

/** A section J shows after another (`CommitReviewCodeProps.nextSection`): its key and its file */
export interface NextSection {
  readonly key: string
  readonly path: string
}

export interface CommitReviewCodeProps {
  ref?: Ref<CommitReviewCodeHandle>
  /** The files in the order of the tree, with their diffs; the texts are read as they come into view */
  files: readonly DiffViewFile[]
  diffViewMode: DiffViewMode
  /** The file shown first, at the top */
  initialPath: string | null
  /** The section whose header is at the top, its key and its file, as the diffs scroll */
  onVisibleSectionChange: (key: string, path: string) => void
  /** The first screen is painted: the window can show */
  onFirstScreen?: () => void
  /** The code owner of a file by its path, in its header */
  fileOwners?: ReadonlyMap<string, string>
  /** The commits of the sides: a declaration outside the review is read at them */
  revisions: DiffRevisions
  /** The files an owner chip marks: the headers of the others are muted; none muted without */
  marked?: ReadonlySet<string> | null
  /** A click on the divider of a step: its first section at the top, as the bar of the step above the diffs */
  onShowStep?: (step: string) => void
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
 * with the plan, a divider above the first section of each step and of each group; a long card and the last one end with
 * a line naming what J shows next.
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
    scrollToSection: key => {
      setFileViews([])
      review.current?.scrollToSection(key)
    },
    moveChange: (direction, accept) => {
      setFileViews([])
      return review.current?.moveChange(direction, accept)
    },
    visibleSection: () => review.current?.visibleSection(),
    focus: () => review.current?.focus(),
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
        renderDivider={section => <ReviewDivider section={section} onShowStep={onShowStep} />}
        renderFooter={
          nextSection === undefined
            ? undefined
            : (key, path) => <ReviewCardEnd path={path} next={nextSection(key)} owner={key === lastKey ? null : ownerLabel} onShow={showFromCardEnd} />
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
            <ReviewFileHeader file={file} notice={null} empty folded={null} collapsed={false} onToggle={() => {}} pending owner={fileOwners?.get(file.path)} muted={false} />
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
 * The divider above the card of a section of the story (spec/branch/review-tour.spec.md, "In the Review Window"): the
 * step it starts, "Step 3 · <title>", which a click shows as the bar of the step does, then the group it starts, muted
 * ("Same edit in 4 files"); its words at the bottom, near the card they head
 */
function ReviewDivider({ section, onShowStep }: { section: DiffSection; onShowStep: ((step: string) => void) | undefined }) {
  const { step, heading } = section
  return (
    // the words at the bottom, near the card they head, on one baseline
    <div className="flex h-full min-w-0 items-end px-1 pb-1.5">
      <div className="flex min-w-0 items-baseline gap-3">
        {step !== null && (
          <button
            type="button"
            data-testid="review-concern-heading"
            data-step={step.id}
            className="min-w-0 truncate rounded-sm text-sm font-semibold underline-offset-4 outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30"
            onClick={() => onShowStep?.(step.id)}
          >
            {step.heading}
          </button>
        )}
        {heading !== null && (
          <span data-testid="review-section-heading" className="shrink-0 text-xs text-muted-foreground">
            {heading}
          </span>
        )}
      </div>
    </div>
  )
}

/** The key of the section of a file of the review (`DiffView`'s `PreparedFile.key`) */
function keyOf(file: DiffViewFile): string {
  return file.section?.key ?? file.file.path
}

/**
 * The line at the end of a long card and of the last one (spec/commit/commit-review.spec.md): what J shows next, its
 * name and its last folder, which a click shows as J does, or the end (`presentCardEnd`); `owner` before the last card,
 * the chosen owner chip whose files J steps through. Not focusable: J does it from the keys
 */
function ReviewCardEnd({ path, next, owner, onShow }: { path: string; next: NextSection | undefined; owner: string | null; onShow: (next: NextSection) => void }) {
  const end = presentCardEnd(path, next?.path, owner)
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
      className="flex h-full min-w-0 items-center gap-1.5 rounded-sm outline-none hover:text-foreground"
      onClick={() => onShow(next)}
    >
      <span className="shrink-0">Next:</span>
      <span className="min-w-0 truncate font-semibold text-foreground">{end.continued ? `${end.name},` : end.name}</span>
      {end.continued ? <span className="shrink-0">continued</span> : end.folder !== null && <span className="min-w-0 shrink-[10] truncate">{end.folder}</span>}
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
function ReviewFileHeader({ file, notice, empty, folded, pending = false, owner, muted, collapsed, onToggle, onExpandAll }: ReviewFileHeaderProps) {
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

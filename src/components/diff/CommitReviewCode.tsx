import { useImperativeHandle, useRef, useState } from "react"
import type { Ref } from "react"
import { UnfoldVerticalIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { DeclarationFileView } from "@/components/diff/DeclarationFileView"
import { DiffView } from "@/components/diff/DiffView"
import type { DeclarationPlace, DiffHeaderProps, DiffViewFile, DiffViewHandle } from "@/components/diff/DiffView"
import { SymbolChooser } from "@/components/diff/SymbolChooser"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FileHeaderChevron, FileHeaderCopyButton } from "@/components/files/FileHeaderParts"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import { placeOf, useSymbolNavigation } from "@/hooks/useSymbolNavigation"
import type { DeclarationTarget, DiffRevisions } from "@/lib/bindings"
import type { CommitFile } from "@/lib/bindings"
import { formatCommitFilePath } from "@/lib/commit-files"
import { useDiffSoftWraps } from "@/stores/settings/diff-view-settings"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/** The files whose headers show while the diffs of a review are read */
const PENDING_HEADERS = 12

export type CommitReviewCodeHandle = DiffViewHandle

export interface CommitReviewCodeProps {
  ref?: Ref<CommitReviewCodeHandle>
  /** The files in the order of the tree, with their diffs; the texts are read as they come into view */
  files: readonly DiffViewFile[]
  diffViewMode: DiffViewMode
  /** The file shown first, at the top */
  initialPath: string | null
  /** The file whose header is at the top, as the diffs scroll */
  onVisibleFileChange: (path: string) => void
  /** The first screen is painted: the window can show */
  onFirstScreen?: () => void
  /** The code owner of a file by its path, in its header */
  fileOwners?: ReadonlyMap<string, string>
  /** The commits of the sides: a declaration outside the review is read at them */
  revisions: DiffRevisions
}

/**
 * The diffs of all files of a commit or a range in one `DiffView`: a sticky header per file, the unchanged lines folded
 * as IntelliJ, lock and generated files and files over 1,000 changed lines collapsed.
 */
export function CommitReviewCode({ ref, files, diffViewMode, initialPath, onVisibleFileChange, onFirstScreen, fileOwners, revisions }: CommitReviewCodeProps) {
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
    moveChange: direction => {
      setFileViews([])
      return review.current?.moveChange(direction)
    },
    focus: () => review.current?.focus(),
    revealDeclaration: (place, from) => review.current?.revealDeclaration(place, from) ?? false,
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
  return (
    <div className="relative h-full">
      <DiffView
        ref={review}
        files={files}
        mode={diffViewMode}
        softWraps={softWraps}
        initialPath={initialPath}
        label="Changes of the commit"
        onVisibleFileChange={onVisibleFileChange}
        onFirstScreen={onFirstScreen}
        onGoToDeclaration={inReview.goToDeclaration}
        onFindUsages={inReview.findUsages}
        renderHeader={header => <ReviewFileHeader {...header} owner={fileOwners?.get(header.file.path)} />}
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
            <ReviewFileHeader file={file} notice={null} empty collapsed={false} onToggle={() => {}} pending owner={fileOwners?.get(file.path)} />
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
}

/**
 * The sticky header of a file, the top of its card: what the tree row says, the name bold and the path muted, Expand
 * All, its owner and counts, and the menu of the file. The insets are IntelliJ's: 12 px above and below, 10 px at the
 * sides
 */
function ReviewFileHeader({ file, notice, empty, pending = false, owner, collapsed, onToggle, onExpandAll }: ReviewFileHeaderProps) {
  const name = file.path.slice(file.path.lastIndexOf("/") + 1)
  return (
    <FileContextMenu
      target={() => ({ kind: "file", path: file.path })}
      selectable
      render={<div data-testid="review-file-header" data-path={file.path} className="group/file-header flex items-center gap-2 px-2.5 text-sm" />}
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

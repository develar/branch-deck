import type { Ref } from "react"
import { cn } from "cn"
import { UnfoldVerticalIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { DiffView } from "@/components/diff/DiffView"
import type { DiffHeaderProps, DiffViewFile, DiffViewHandle } from "@/components/diff/DiffView"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FILE_HEADER_CLASS_NAME, FileHeaderChevron, FileHeaderCopyButton } from "@/components/files/FileHeaderParts"
import { FilePathLabel } from "@/components/files/FilePathLabel"
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
}

/**
 * The diffs of all files of a commit or a range in one `DiffView`: a sticky header per file, the unchanged lines folded
 * as IntelliJ, lock and generated files and files over 1,000 changed lines collapsed.
 */
export function CommitReviewCode({ ref, files, diffViewMode, initialPath, onVisibleFileChange, onFirstScreen, fileOwners }: CommitReviewCodeProps) {
  const softWraps = useDiffSoftWraps()
  return (
    <DiffView
      ref={ref}
      files={files}
      mode={diffViewMode}
      softWraps={softWraps}
      initialPath={initialPath}
      label="Changes of the commit"
      onVisibleFileChange={onVisibleFileChange}
      onFirstScreen={onFirstScreen}
      renderHeader={header => <ReviewFileHeader {...header} owner={fileOwners?.get(header.file.path)} />}
    />
  )
}

/**
 * The headers of the first files while their patch loads: where the diffs will be, in the order of the tree, so the
 * window that shows before a slow git has its files on screen
 */
export function PendingFileHeaders({ files, fileOwners }: { files: readonly CommitFile[]; fileOwners?: ReadonlyMap<string, string> }) {
  return (
    <div data-testid="review-pending-files" aria-busy="true" className="flex flex-col gap-2">
      {files.slice(0, PENDING_HEADERS).map(file => (
        <ReviewFileHeader key={file.path} file={file} notice={null} empty collapsed={false} onToggle={() => {}} pending owner={fileOwners?.get(file.path)} />
      ))}
    </div>
  )
}

interface ReviewFileHeaderProps extends DiffHeaderProps {
  /** The diffs are not read yet: no notice, nothing to collapse */
  pending?: boolean
  owner: string | undefined
}

/** The sticky header of a file: what the tree row says, with the path in full, its owner, and the menu of the file */
function ReviewFileHeader({ file, notice, empty, pending = false, owner, collapsed, onToggle, onExpandAll }: ReviewFileHeaderProps) {
  const name = file.path.slice(file.path.lastIndexOf("/") + 1)
  return (
    <FileContextMenu
      target={() => ({ kind: "file", path: file.path })}
      selectable
      render={<div data-testid="review-file-header" data-path={file.path} className={cn(FILE_HEADER_CLASS_NAME, "border-b border-border bg-card")} />}
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
      {onExpandAll !== undefined && (
        <Button variant="ghost" size="icon-xs" aria-label={`Expand all unchanged lines of ${name}`} title="Expand All" onClick={onExpandAll}>
          <UnfoldVerticalIcon />
        </Button>
      )}
      <span className="ml-auto flex shrink-0 items-center gap-3 text-xs">
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

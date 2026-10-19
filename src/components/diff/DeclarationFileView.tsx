import { ArrowLeftIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { DiffView } from "@/components/diff/DiffView"
import type { DeclarationPlace, DiffViewFile } from "@/components/diff/DiffView"
import type { DeclarationSite, DeclarationTarget } from "@/lib/bindings"
import type { FileDiff } from "@/lib/diff/file-diff-model"
import { isMac } from "@/lib/platform"
import { useDiffSoftWraps } from "@/stores/settings/diff-view-settings"

/** A file outside the review: a diff that adds it whole, which the view shows plain */
function fileOf(target: DeclarationTarget): DiffViewFile {
  const { blobId, lineCount } = target.outside!
  const diff: FileDiff = {
    path: target.path,
    oldPath: null,
    status: "added",
    oldId: null,
    newId: blobId,
    content: {
      kind: "text",
      oldLineCount: 1,
      newLineCount: lineCount,
      additions: lineCount,
      deletions: 0,
      lineEnds: null,
      finalNewline: null,
      emptyLastLine: { old: true, new: true },
      lineLevelOnly: false,
      blocks: [{ oldStart: 0, oldEnd: 0, newStart: 0, newEnd: lineCount, kind: "changed", inlinePairs: false, oldRanges: [], newRanges: [] }],
      oldScopes: [],
      newScopes: [],
    },
  }
  return {
    file: { path: target.path, status: "added", oldPath: null, additions: lineCount, deletions: 0, binary: false, oldId: null, newId: blobId, submodule: false },
    diff,
  }
}

export interface DeclarationFileViewProps {
  target: DeclarationTarget
  /** Back to the review, or to the file view before this one */
  onBack: () => void
  onGoToDeclaration: (site: DeclarationSite, at: { x: number; y: number }, reveal: (place: DeclarationPlace) => boolean, from: DeclarationPlace) => void
  onFindUsages: (site: DeclarationSite, at: { x: number; y: number }, reveal: (place: DeclarationPlace) => boolean, from: DeclarationPlace) => void
}

/**
 * A file outside the review that holds a declaration (spec/commit/symbol-navigation.spec.md): the whole file at the
 * revision of its side, coloured and navigable as the diffs, over the review, which keeps its place under it
 */
export function DeclarationFileView({ target, onBack, onGoToDeclaration, onFindUsages }: DeclarationFileViewProps) {
  const softWraps = useDiffSoftWraps()
  const revision = target.outside?.revision
  const place: DeclarationPlace = { path: target.path, side: "new", line: target.line, column: target.column, endColumn: target.endColumn }
  return (
    <div data-testid="declaration-file-view" className="absolute inset-0 z-10 flex flex-col bg-canvas">
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-1.5 text-sm">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ArrowLeftIcon data-icon="inline-start" />
          Back
          <Kbd>{isMac ? "⌘[" : "Ctrl+Alt+←"}</Kbd>
        </Button>
        <span className="min-w-0 truncate font-medium" title={target.path}>
          {target.path}
        </span>
        <span className="shrink-0 text-muted-foreground">{revision == null ? "the working tree" : `at ${revision.slice(0, 8)}`}, outside the review</span>
      </div>
      <div className="min-h-0 flex-1">
        <DiffView
          files={[fileOf(target)]}
          mode="unified"
          softWraps={softWraps}
          initialPath={target.path}
          initialPlace={place}
          label={`File ${target.path}`}
          plain
          onGoToDeclaration={onGoToDeclaration}
          onFindUsages={onFindUsages}
          onHistoryEnd={onBack}
        />
      </div>
    </div>
  )
}

import type { ReactNode } from "react"
import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ContractParagraphView, ContractTableView } from "@/components/review/ContractParagraphView"
import { BreakableCode } from "@/components/shared/CodeText"
import { Disclosure } from "@/components/shared/Disclosure"
import { SectionLabel } from "@/components/shared/SectionLabel"
import { useSelectReviewFile } from "@/hooks/useSelectReviewFile"
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"
import type { ContractFile, SubstitutionView } from "@/presenters/review-story"

/**
 * What Changes (openspec/specs/branch/review-tour/spec.md, "Brief"): the repeated replacements once, then each file of prose the
 * change touches, its kind and path once, each section its heading, its tests and its paragraphs and table rows behind
 * a muted quote line. In the overview of a review whose steps are not told, else in the opening of each step for the
 * entries that step changes (`OpeningView.contract`)
 */
export function ContractList({
  contract,
  substitutions = [],
  title = "What Changes",
  foldPast = null,
}: {
  contract: readonly ContractFile[]
  substitutions?: readonly SubstitutionView[]
  title?: string | null
  /** Of the opening of a step: more blocks than this fold each file under the line of its sections, its header kept */
  foldPast?: number | null
}) {
  if (contract.length === 0 && substitutions.length === 0) {
    return null
  }
  const blocks = contract.reduce((sum, file) => sum + file.entries.reduce((count, entry) => count + entry.blocks.length, 0), 0)
  const folds = foldPast !== null && blocks > foldPast
  return (
    <div className="flex flex-col gap-2" data-testid="review-brief-contract">
      {title !== null && <SectionLabel title={title} className="text-sm font-semibold text-foreground" />}
      {substitutions.map(substitution => (
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
        {contract.map(file => (
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
            {folds ? (
              <Disclosure tone="muted" title={file.summary} testId="review-brief-contract-fold" className="text-sm">
                <ContractQuotes file={file} />
              </Disclosure>
            ) : (
              <ContractQuotes file={file} />
            )}
          </li>
        ))}
      </ul>
    </div>
  )
}

/** The sections of a file of What Changes behind their quote line: each its heading once, its tests, its paragraphs and table rows */
function ContractQuotes({ file }: { file: ContractFile }) {
  return (
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
            block.kind === "table" ? <ContractTableView key={block.key} table={block} /> : <ContractParagraphView key={block.paragraph.key} paragraph={block.paragraph} />,
          )}
        </div>
      ))}
    </div>
  )
}

/**
 * What a click shows in the review: a changed file, by its path or its old path, at the line of the entry; plain text
 * outside a review
 */
export function FileButton({ place, title, testId = "review-plan-file", children }: { place: ReviewPlace; title: string; testId?: string; children: ReactNode }) {
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

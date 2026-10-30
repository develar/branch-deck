import { useState } from "react"
import type { MouseEvent, ReactNode } from "react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Popover, PopoverContent, PopoverSectionTitle, PopoverTrigger } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { DependencyBadge, FileBadges, LinkEntry } from "@/presenters/review-plan"

/*
 * The badges of a row of the tree of a review with its plan (spec/branch/review-plan.spec.md, In the Review Window):
 * the links of a file among the changed files and their popover, the uses its API leaves outside the change, no test,
 * the same edit as other files, and the dependency changes of a module. The presenter words them; a click on a file in
 * a popover selects it.
 */

/** The badge size of a tree row: a row is `h-6` */
const ROW_BADGE = "h-4 px-1.5"

/** A click in a badge acts on the badge only: the row under it does not select or toggle */
function keepFromRow(event: MouseEvent) {
  event.stopPropagation()
}

/** The badges of a file row; `onSelectPath` selects another changed file */
export function FileRowBadges({ badges, onSelectPath }: { badges: FileBadges; onSelectPath: (path: string) => void }) {
  return (
    <span className="flex shrink-0 items-center gap-1" onClick={keepFromRow} data-testid="file-badges">
      {(badges.usesLinks.length > 0 || badges.usedByLinks.length > 0) && <FileLinksPopover badges={badges} onSelectPath={onSelectPath} />}
      {badges.breaks !== null && (
        <Badge
          variant={badges.breaks.tone === "warning" ? "warning" : "outline"}
          className={cn(ROW_BADGE, badges.breaks.tone === "muted" && "text-muted-foreground")}
          title={badges.breaks.title}
          data-testid="file-breaks-badge"
        >
          {badges.breaks.label}
        </Badge>
      )}
      {badges.sameEdit !== null && <SameEditPopover sameEdit={badges.sameEdit} onSelectPath={onSelectPath} />}
      {badges.untested !== null && (
        <span className="text-xs text-muted-foreground" title="No test of this file changed" data-testid="file-untested-badge">
          {badges.untested}
        </span>
      )}
    </span>
  )
}

/** A badge that opens a popover of files: a click on one selects it and closes the popover */
function FilesPopover({ label, testId, children }: { label: ReactNode; testId: string; children: (close: () => void) => ReactNode }) {
  const [open, setOpen] = useState(false)
  return (
    <Popover lazy open={open} onOpenChange={setOpen}>
      <PopoverTrigger render={<Badge render={<button type="button" />} variant="outline" className={cn(ROW_BADGE, "cursor-pointer hover:bg-muted")} data-testid={testId} />}>
        {label}
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 gap-3 p-3" data-testid={`${testId}-popover`}>
        {children(() => setOpen(false))}
      </PopoverContent>
    </Popover>
  )
}

/** A changed file in a popover: its name, what it says of it; a click selects it */
function PopoverFile({
  path,
  name,
  detail,
  aside,
  onSelect,
  testId,
  state,
}: {
  path: string
  name: string
  detail: string
  aside?: string
  onSelect: () => void
  testId: string
  state?: string
}) {
  return (
    <button
      type="button"
      className="flex w-full min-w-0 flex-col items-start gap-0.5 rounded-md px-2 py-1 text-left hover:bg-muted focus-visible:bg-muted focus-visible:outline-none"
      title={path}
      onClick={onSelect}
      data-testid={testId}
      data-path={path}
      data-state={state}
    >
      <span className="flex w-full min-w-0 items-center gap-2">
        <span className="min-w-0 flex-1 truncate">{name}</span>
        {aside !== undefined && <span className="shrink-0 text-xs text-muted-foreground">{aside}</span>}
      </span>
      <span className="w-full min-w-0 truncate font-mono text-xs text-muted-foreground">{detail}</span>
    </button>
  )
}

/** "uses 2 +1 new · used by 1": the links of the file among the changed files; its popover lists each link */
function FileLinksPopover({ badges, onSelectPath }: { badges: FileBadges; onSelectPath: (path: string) => void }) {
  const parts = [badges.uses, badges.usedBy].filter(part => part !== null)
  const label = (
    <>
      {parts.length === 0 ? "links" : parts.join(" · ")}
      {badges.newLinks !== null && <span className="text-success">{badges.newLinks}</span>}
    </>
  )
  const section = (title: string, links: readonly LinkEntry[], close: () => void) =>
    links.length > 0 && (
      <div className="flex flex-col gap-1">
        <PopoverSectionTitle className="px-2">{title}</PopoverSectionTitle>
        {links.map(link => (
          <PopoverFile
            key={link.key}
            path={link.path}
            name={link.name}
            detail={link.symbols}
            aside={link.stateWord}
            state={link.state}
            testId="file-link"
            onSelect={() => {
              close()
              onSelectPath(link.path)
            }}
          />
        ))}
      </div>
    )
  return (
    <FilesPopover label={label} testId="file-links-badge">
      {close => (
        <>
          {section("Uses", badges.usesLinks, close)}
          {section("Used by", badges.usedByLinks, close)}
        </>
      )}
    </FilesPopover>
  )
}

/** "same edit ×4": the files that get the same edit; its popover lists the others */
function SameEditPopover({ sameEdit, onSelectPath }: { sameEdit: NonNullable<FileBadges["sameEdit"]>; onSelectPath: (path: string) => void }) {
  return (
    <FilesPopover label={sameEdit.label} testId="file-same-edit-badge">
      {close => (
        <div className="flex flex-col gap-1">
          <PopoverSectionTitle className="px-2">Same edit</PopoverSectionTitle>
          <p className="truncate px-2 font-mono text-xs" title={sameEdit.edit} data-testid="file-same-edit">
            {sameEdit.edit}
          </p>
          {sameEdit.files.map(path => (
            <PopoverFile
              key={path}
              path={path}
              name={path.slice(path.lastIndexOf("/") + 1)}
              detail={path}
              testId="file-same-edit-file"
              onSelect={() => {
                close()
                onSelectPath(path)
              }}
            />
          ))}
        </div>
      )}
    </FilesPopover>
  )
}

/** "+2 deps −1 ~1" of the row of a module: its tooltip lists each change of its build files */
export function DependencyBadgeView({ badge }: { badge: DependencyBadge }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<span />} onClick={keepFromRow}>
        <Badge variant="info" className={ROW_BADGE} data-testid="directory-dependency-badge">
          {badge.label}
        </Badge>
      </TooltipTrigger>
      <TooltipContent>
        <span className="flex flex-col" data-testid="directory-dependency-changes">
          {badge.lines.map((line, index) => (
            // two changes can read the same: the same name in two scopes of one module
            <span key={index}>{line}</span>
          ))}
        </span>
      </TooltipContent>
    </Tooltip>
  )
}

import { cn } from "cn"
import { AlignLeftIcon, ChevronDownIcon, Columns2Icon, PilcrowIcon, StrikethroughIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DropdownMenu, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { FieldGroup } from "@/components/layout/FieldGroup"
import type { WhitespacePolicy } from "@/lib/diff/file-diff-model"
import { setDiffViewMode, setDiffWhitespacePolicy, useDiffViewMode, useDiffWhitespacePolicy } from "@/stores/settings/diff-view-settings"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/** The layouts in IntelliJ's order: side by side (the default), unified, then Inline beside it as its compact variant */
const modes: Array<{ mode: DiffViewMode; label: string; icon: typeof AlignLeftIcon }> = [
  { mode: "split", label: "Split", icon: Columns2Icon },
  { mode: "unified", label: "Unified", icon: AlignLeftIcon },
  { mode: "inline", label: "Inline", icon: StrikethroughIcon },
]

/** IntelliJ's choices of what the diff ignores, in its order */
const WHITESPACE_POLICIES: Array<{ policy: WhitespacePolicy; label: string }> = [
  { policy: "default", label: "Do Not Ignore" },
  { policy: "trim", label: "Trim Whitespace" },
  { policy: "ignore", label: "Ignore Whitespace" },
  { policy: "ignoreBlankLines", label: "Ignore Whitespace and Blank Lines" },
]

/** The layout switch of every diff view (`diff-view-settings.ts`) and the Whitespace menu beside it, above the diffs */
export function DiffViewToggle() {
  return (
    <div className="flex items-center gap-2">
      <DiffLayoutButtons />
      <WhitespaceMenu />
    </div>
  )
}

/** Split · Unified · Inline, the pressed one the layout of every diff view; also in Settings → Diffs & Merge */
export function DiffLayoutButtons() {
  const current = useDiffViewMode()
  return (
    <FieldGroup>
      {modes.map(({ mode, label, icon: Icon }) => (
        <Button
          key={mode}
          size="xs"
          variant="outline"
          aria-pressed={current === mode}
          className={cn(current === mode && "border-primary/50 text-primary hover:text-primary")}
          onClick={() => setDiffViewMode(mode)}
        >
          <Icon data-icon="inline-start" />
          {label}
        </Button>
      ))}
    </FieldGroup>
  )
}

/**
 * "Whitespace": what the diff engine ignores; the button is marked while it ignores anything. `named`: the button
 * says the choice instead, as a row of Settings → Diffs & Merge labelled "Whitespace"
 */
export function WhitespaceMenu({ named = false }: { named?: boolean }) {
  const policy = useDiffWhitespacePolicy()
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button size="xs" variant="outline" className={cn(!named && policy !== "default" && "border-primary/50 text-primary hover:text-primary")} />}>
        <PilcrowIcon data-icon="inline-start" />
        {named ? WHITESPACE_POLICIES.find(it => it.policy === policy)?.label : "Whitespace"}
        <ChevronDownIcon data-icon="inline-end" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup value={policy} onValueChange={value => setDiffWhitespacePolicy(value as WhitespacePolicy)}>
          {WHITESPACE_POLICIES.map(({ policy: value, label }) => (
            <DropdownMenuRadioItem key={value} value={value}>
              {label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

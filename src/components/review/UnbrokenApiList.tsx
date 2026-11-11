import { Badge } from "@/components/ui/badge"
import { FileButton } from "@/components/review/ContractList"
import { StartTruncatedText } from "@/components/shared/StartTruncatedText"
import type { BreakageEntry } from "@/presenters/review-plan"

/**
 * The API none of whose uses left outside breaks, a line each: its badge ("Opens wider", "Changed"), its name and its
 * `path:line` muted after it, a click showing its declaration; under the verdict on the line of the map, else under
 * the entries of Possible Breakage
 */
export function UnbrokenApiList({ entries }: { entries: readonly BreakageEntry[] }) {
  return (
    <ul aria-label="API changes no use left outside breaks" className="flex flex-col gap-0.5 text-xs">
      {entries.map(entry => (
        <li key={entry.key} className="flex min-w-0 items-center gap-2" data-testid="review-plan-unbroken-entry" data-name={entry.name}>
          <Badge variant={entry.tone} className="shrink-0">
            {entry.kind}
          </Badge>
          <FileButton place={entry.place} title={entry.location}>
            <code className="shrink-0 font-mono">{entry.name}</code>
            <StartTruncatedText className="text-xs text-muted-foreground">{entry.location}</StartTruncatedText>
          </FileButton>
        </li>
      ))}
    </ul>
  )
}

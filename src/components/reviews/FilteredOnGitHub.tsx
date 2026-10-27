import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { openPullRequestUrl } from "@/lib/pull-request"

/**
 * "First 50 of 63 filtered · Search on GitHub": the filter of Reviews sees only what GitHub returned of a list, so the
 * list's search on GitHub with the typed words finds the rest
 */
export function FilteredOnGitHub({ count, totalCount, url, className }: { count: number; totalCount: number; url: string | null; className?: string }) {
  return (
    <span className={cn("flex items-center gap-2 text-xs text-muted-foreground", className)} data-testid="reviews-filtered-on-github">
      {`First ${count} of ${totalCount} filtered`}
      {url !== null && (
        <Button variant="link" size="xs" className="h-auto px-0" onClick={() => openPullRequestUrl(url)}>
          Search on GitHub
        </Button>
      )}
    </span>
  )
}

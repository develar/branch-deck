import { Spinner } from "@/components/ui/spinner"

/**
 * The place of a section whose fact is still to come (the pull request of a branch, its Safe Push): one muted line with
 * a spinner in the box of the section, so the answer fills a place that is there already instead of pushing what is
 * below it down
 */
export function PendingSection({ label, text, testId }: { label: string; text: string; testId: string }) {
  return (
    <section aria-label={label} aria-busy="true" className="rounded-lg border border-border p-3" data-testid={testId}>
      <p className="flex items-center gap-2 text-xs text-muted-foreground">
        <Spinner className="size-3.5" />
        {text}
      </p>
    </section>
  )
}

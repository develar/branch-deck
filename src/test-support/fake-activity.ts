// The journals of Activity of a unit test, in memory: `readActivity`, `recordActivity` and `clearActivity` of the mocked
// bindings, as the backend answers them (`operations::journal`): the page entries it records, the journal of the
// repository a test sets (`fakeActivity.journals`). A test mocks the commands with them:
// `commands: { ...original.commands, ...fakeActivity.commands }`, and resets the journals with `fakeActivity.reset()`.
import type { ActivityLog, ClearActivityParams, PageActivity, PageEntry, RepositoryJournal } from "@/lib/bindings"

type Answer<T> = { status: "ok"; data: T }

function ok<T>(data: T): Answer<T> {
  return { status: "ok", data }
}

const page: PageEntry[] = []
let journals: RepositoryJournal[] = []
let nextId = 0

export const fakeActivity = {
  /** The entries the page recorded, oldest first */
  page,
  /** The journals of the repositories `readActivity` answers */
  setJournals(next: RepositoryJournal[]): void {
    journals = next
  },
  reset(): void {
    page.length = 0
    journals = []
  },
  commands: {
    readActivity: async (): Promise<Answer<ActivityLog>> => ok({ journals: journals.map(journal => ({ ...journal, entries: [...journal.entries] })), page: [...page] }),
    recordActivity: async (activity: PageActivity): Promise<Answer<PageEntry>> => {
      nextId++
      const entry: PageEntry = { id: `page-${nextId}`, endedAt: Math.floor(Date.now() / 1000), activity }
      page.push(entry)
      return ok(entry)
    },
    clearActivity: async ({ repositoryPath }: ClearActivityParams): Promise<Answer<null>> => {
      const kept = page.filter(entry => entry.activity.repositoryPath !== repositoryPath)
      page.splice(0, page.length, ...kept)
      journals = journals.map(journal => (journal.repositories.includes(repositoryPath) ? { ...journal, entries: [] } : journal))
      return ok(null)
    },
  },
}

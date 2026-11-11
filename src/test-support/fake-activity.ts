// The journals of Activity of a unit test, in memory: `readActivity`, `recordActivity` and `clearActivity` of the mocked
// bindings, as the backend answers them (`operations::journal`): the page entries it records, the journal of the
// repository a test sets (`fakeActivity.journals`). A test mocks the commands with them:
// `commands: { ...original.commands, ...fakeActivity.commands }`, and resets the journals with `fakeActivity.reset()`.
// `holdAnswer` holds the answer of the next call of a command, which reads or writes at once, as the backend does: the
// answers of two calls come in another order than their work.
import type { ActivityLog, ClearActivityParams, PageActivity, PageEntry, RepositoryJournal } from "@/lib/bindings"

type Answer<T> = { status: "ok"; data: T }

function ok<T>(data: T): Answer<T> {
  return { status: "ok", data }
}

const page: PageEntry[] = []
let journals: RepositoryJournal[] = []
let nextId = 0

type HeldCommand = "readActivity" | "recordActivity"

/** The answer each command holds back, of its next call, until its release */
const held = new Map<HeldCommand, Promise<void>>()

/** Waits for the release of the answer of `command`, when its next call was held */
async function answered(command: HeldCommand): Promise<void> {
  const release = held.get(command)
  held.delete(command)
  await release
}

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
    held.clear()
  },
  /** Holds the answer of the next call of `command`: it reads or writes at once and answers once the release is called */
  holdAnswer(command: HeldCommand): () => void {
    let release!: () => void
    held.set(command, new Promise<void>(resolve => (release = resolve)))
    return release
  },
  commands: {
    readActivity: async (): Promise<Answer<ActivityLog>> => {
      const log = { journals: journals.map(journal => ({ ...journal, entries: [...journal.entries] })), page: [...page] }
      await answered("readActivity")
      return ok(log)
    },
    recordActivity: async (activity: PageActivity): Promise<Answer<PageEntry>> => {
      nextId++
      const entry: PageEntry = { id: `page-${nextId}`, endedAt: Math.floor(Date.now() / 1000), activity }
      page.push(entry)
      await answered("recordActivity")
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

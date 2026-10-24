/** Error caused by missing user input or configuration; shown to the user as is, without "internal error" framing. */
export class UserError extends Error {}

/** The repository a command acts on; Rust resolves the branch prefix of its branches for every command */
export interface VcsRequest {
  readonly repositoryPath: string
}

/**
 * Builds a {@link VcsRequest} from the current repository state.
 * Takes a getter rather than a value, so a single instance always reads the latest selection.
 */
export class VcsRequestFactory {
  constructor(private readonly getRepositoryPath: () => string) {}

  /** @throws UserError if no repository is selected */
  createRequest(): VcsRequest {
    const repositoryPath = this.getRepositoryPath().trim()
    if (!repositoryPath) {
      throw new UserError("Please enter a repository path")
    }
    return { repositoryPath }
  }
}

/**
 * The key a store keeps the state of the repository at `repositoryPath` under (`<path>\n<prefix>`, the prefix the page
 * names its branches with): `current`, the selected repository's, when it is of that repository, else one of `kept`
 * stored while it was selected (the answer of an action applies to the repository it was asked in, also after a
 * switch); null for a repository the store keeps nothing of
 */
export function storeKeyOf(repositoryPath: string, current: string | null, kept: Iterable<string>): string | null {
  const ofRepository = (key: string) => key.startsWith(`${repositoryPath}\n`)
  if (current !== null && ofRepository(current)) {
    return current
  }
  for (const key of kept) {
    if (ofRepository(key)) {
      return key
    }
  }
  return null
}

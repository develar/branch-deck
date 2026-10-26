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

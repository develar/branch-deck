/** Error caused by missing user input or configuration; shown to the user as is, without "internal error" framing. */
export class UserError extends Error {}

export interface VcsRequest {
  readonly repositoryPath: string
  readonly branchPrefix: string
}

/**
 * Builds a {@link VcsRequest} from the current repository state.
 * Takes getters rather than values, so a single instance always reads the latest selection.
 */
export class VcsRequestFactory {
  constructor(
    private readonly getRepositoryPath: () => string,
    private readonly getBranchPrefix: () => string,
  ) {}

  /** @throws UserError if the repository path or the branch prefix is not configured */
  createRequest(): VcsRequest {
    return createVcsRequest(this.getRepositoryPath(), this.getBranchPrefix())
  }
}

function createVcsRequest(repositoryPath: string, branchPrefix: string): VcsRequest {
  if (!repositoryPath.trim()) {
    throw new UserError("Please enter a repository path")
  }
  if (!branchPrefix) {
    throw new UserError("Please configure a branch prefix")
  }

  return {
    repositoryPath: repositoryPath.trim(),
    branchPrefix,
  }
}

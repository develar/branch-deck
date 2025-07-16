import type { Ref } from "vue"

export class VcsRequestFactory {
  constructor(
    private readonly repositoryPath: Ref<string>,
    private readonly branchPrefix: Ref<string>,
  ) {
  }

  createRequest(): VcsRequest {
    return createVcsRequest(this.repositoryPath.value, this.branchPrefix.value)
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
    branchPrefix: branchPrefix,
  }
}

export class UserError extends Error {}

export interface VcsRequest {
  readonly repositoryPath: string
  readonly branchPrefix: string
}

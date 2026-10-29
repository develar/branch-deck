/** The repository a command acts on; Rust resolves the branch prefix of its branches for every command */
export interface VcsRequest {
  readonly repositoryPath: string
}

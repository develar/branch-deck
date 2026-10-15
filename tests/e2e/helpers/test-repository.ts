import type { BranchNameSuggestionOutcome, PatronusRobotsResult, PullRequestsResult } from "../../../src/lib/bindings"

export class TestRepositoryBuilder {
  private baseUrl = "http://localhost:3030"
  public id: string = ""
  public path: string = ""
  private template: string | null = null
  private prepopulateStore: boolean = true

  useTemplate(templateName: string): this {
    this.template = templateName
    return this
  }

  withPrepopulateStore(prepopulate: boolean): this {
    this.prepopulateStore = prepopulate
    return this
  }

  async init(): Promise<void> {
    if (!this.template) {
      throw new Error("Template is required. Use useTemplate() before init()")
    }

    // Create a new test repository
    const response = await fetch(`${this.baseUrl}/repositories`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        template: this.template,
        prepopulate_store: this.prepopulateStore,
      }),
    })

    if (!response.ok) {
      throw new Error(`Failed to create repository: ${response.statusText}`)
    }

    const data = await response.json()
    this.id = data.id
    this.path = data.path
  }

  /** Rewrites the message of a commit behind the app's back (as `git rebase -i` would); returns the new commit id. */
  async reword(commitId: string, message: string): Promise<string> {
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/reword`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ commitId, message }),
    })

    if (!response.ok) {
      throw new Error(`Failed to reword commit ${commitId}: ${response.statusText}`)
    }

    const data = await response.json()
    return data.newCommitId
  }

  /** Sets what `get_pull_requests` answers from now on (E2E never runs gh) */
  async setPullRequests(result: PullRequestsResult): Promise<void> {
    await this.setFixture("pull-requests", result)
  }

  /** How often the app asked for pull requests */
  async getPullRequestCalls(): Promise<number> {
    return (await this.getFixture<{ calls: number }>("pull-requests")).calls
  }

  /** Sets what `suggest_branch_names` answers from now on and how long it takes (E2E never runs claude) */
  async setBranchNameSuggestions(fixture: { outcome?: BranchNameSuggestionOutcome, delayMs?: number }): Promise<void> {
    await this.setFixture("branch-name-suggestions", fixture)
  }

  /** How often the app asked for branch names */
  async getBranchNameSuggestionCalls(): Promise<number> {
    return (await this.getFixture<{ calls: number }>("branch-name-suggestions")).calls
  }

  /** Sets what `get_patronus_robots` and `poll_patronus_robots` answer from now on (E2E never calls Patronus) */
  async setPatronus(result: PatronusRobotsResult): Promise<void> {
    await this.setFixture("patronus", result)
  }

  /** How often the app listed and polled the Patronus robots */
  async getPatronusCalls(): Promise<{ listCalls: number, pollCalls: number }> {
    return this.getFixture("patronus")
  }

  private async setFixture(name: string, value: unknown): Promise<void> {
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/${name}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(value),
    })
    if (!response.ok) {
      throw new Error(`Failed to set the ${name} fixture: ${response.status} ${await response.text()}`)
    }
  }

  private async getFixture<T>(name: string): Promise<T> {
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/${name}`)
    return await response.json() as T
  }

  async cleanup(): Promise<void> {
    if (!this.id) {
      return
    }

    try {
      // Delete the repository from the test server
      const response = await fetch(`${this.baseUrl}/repositories/${this.id}`, {
        method: "DELETE",
      })

      if (!response.ok) {
        console.warn(`Failed to cleanup repository ${this.id}: ${response.statusText}`)
      }
    }
    catch (error) {
      // Log but don't throw - cleanup errors shouldn't fail tests
      console.warn(`Failed to cleanup repository ${this.id}:`, error)
    }
  }
}

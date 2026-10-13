export class TestRepositoryBuilder {
  private baseUrl = "http://localhost:3030"
  public id: string = ""
  public path: string = ""
  private template: string | null = null
  private prepopulateStore: boolean = true
  private modelState: "not_downloaded" | "downloaded" | "downloading" | null = null

  useTemplate(templateName: string): this {
    this.template = templateName
    return this
  }

  withPrepopulateStore(prepopulate: boolean): this {
    this.prepopulateStore = prepopulate
    return this
  }

  withModelState(state: "not_downloaded" | "downloaded" | "downloading"): this {
    this.modelState = state
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
        ...(this.modelState && { model_state: this.modelState }),
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

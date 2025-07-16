interface BranchConfig {
  name: string
  commits: CommitConfig[]
}

interface CommitConfig {
  message: string
  files: Record<string, string>
}

export class TestRepositoryBuilder {
  private baseUrl = "http://localhost:3030"
  public id: string = ""
  public path: string = ""
  private branches: BranchConfig[] = []
  private currentBranch: BranchConfig | null = null
  private template: string | null = null

  useTemplate(templateName: string): this {
    this.template = templateName
    return this
  }

  async init(): Promise<void> {
    // Create a new test repository
    const body = this.template ? { template: this.template } : {}
    const response = await fetch(`${this.baseUrl}/repositories`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    })

    if (!response.ok) {
      throw new Error(`Failed to create repository: ${response.statusText}`)
    }

    const data = await response.json()
    this.id = data.id
    this.path = data.path
  }

  branch(name: string): this {
    this.currentBranch = {
      name,
      commits: [],
    }
    this.branches.push(this.currentBranch)
    return this
  }

  commit(message: string, files: Record<string, string>): this {
    if (!this.currentBranch) {
      // If no branch is set, create commits on master
      this.currentBranch = {
        name: "master",
        commits: [],
      }
      // Check if master branch already exists in branches array
      const existingMaster = this.branches.find(b => b.name === "master")
      if (!existingMaster) {
        this.branches.push(this.currentBranch)
      }
      else {
        this.currentBranch = existingMaster
      }
    }

    this.currentBranch.commits.push({ message, files })
    return this
  }

  async build(): Promise<void> {
    if (!this.id) {
      throw new Error("Must call init() before build()")
    }

    // Set up the repository with branches and commits
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/setup`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        branches: this.branches,
      }),
    })

    if (!response.ok) {
      throw new Error(`Failed to setup repository: ${response.statusText}`)
    }
  }

  async cleanup(): Promise<void> {
    if (!this.id) {
      return
    }

    // Delete the repository from the test server
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}`, {
      method: "DELETE",
    })

    if (!response.ok) {
      console.warn(`Failed to cleanup repository ${this.id}: ${response.statusText}`)
    }
  }
}
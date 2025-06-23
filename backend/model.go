package backend

// BranchResult represents the result of creating/updating a branch
type BranchResult struct {
  Name          string         `json:"name"`
  Action        string         `json:"action"`
  CommitCount   int            `json:"commitCount"`
  CommitDetails []CommitDetail `json:"commitDetails"`
  Error         string         `json:"error,omitempty"`
}

// CommitDetail represents details about a commit
type CommitDetail struct {
  Hash    string `json:"hash"`
  Message string `json:"message"`
  IsNew   bool   `json:"isNew"`
}

// ProcessResult represents the overall result
type ProcessResult struct {
  Success  bool           `json:"success"`
  Message  string         `json:"message"`
  Branches []BranchResult `json:"branches"`
  Error    string         `json:"error,omitempty"`
}

// RepositoryInfo represents repository information
type RepositoryInfo struct {
  Path          string   `json:"path"`
  CurrentBranch string   `json:"currentBranch"`
  Remotes       []string `json:"remotes"`
  Error         string   `json:"error,omitempty"`
}

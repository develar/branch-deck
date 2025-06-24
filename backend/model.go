package backend

import (
  "errors"
  "strings"
)

func toFinalBranchName(branchPrefix string, branchName string) string {
  return strings.TrimSuffix(branchPrefix, "/") + "/virtual/" + branchName
}

// BranchResult represents the result of creating/updating a branch
type BranchResult struct {
  Name          string           `json:"name"`
  SyncStatus    BranchSyncStatus `json:"syncStatus,omitempty"`
  CommitCount   int              `json:"commitCount"`
  CommitDetails []CommitDetail   `json:"commitDetails"`
  Error         string           `json:"error,omitempty"`
}

type BranchSyncStatus int

const (
  BranchCreated   BranchSyncStatus = iota
  BranchUpdated   BranchSyncStatus = iota
  BranchUnchanged BranchSyncStatus = iota
)

var AllBranchSyncStatuses = []struct {
  Value  BranchSyncStatus
  TSName string
}{
  {BranchCreated, "CREATED"},
  {BranchUpdated, "UPDATED"},
  {BranchUnchanged, "UNCHANGED"},
}

// CommitDetail represents details about a commit
type CommitDetail struct {
  Hash    string `json:"hash"`
  Message string `json:"message"`
  IsNew   bool   `json:"isNew"`
}

type VcsRequest struct {
  RepositoryPath string `repositoryPath:"string"`
  BranchPrefix   string `branchPrefix:"message"`
}

func (t *VcsRequest) Validate() error {
  if t.RepositoryPath == "" {
    return errors.New("repository path is required")
  }

  if t.BranchPrefix == "" {
    return errors.New("branch prefix is required")
  }
  return nil
}

// ActionResult represents the overall result
type ActionResult struct {
  Success  bool           `json:"success"`
  Message  string         `json:"message,omitempty"`
  Branches []BranchResult `json:"branches"`
}

type GlobalBranchPrefix struct {
  BranchPrefix string `json:"branchPrefix"`
  Error        string `json:"error,omitempty"`
}

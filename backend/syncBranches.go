package backend

import (
  "fmt"
  "regexp"
  "sort"

  "github.com/go-git/go-git/v5"
  "github.com/go-git/go-git/v5/plumbing"
  "github.com/maruel/natural"
)

func CreateBranches(branchPrefix string, gitExecutor *Git) ([]BranchResult, error) {
  // Open the Git repository
  repo, err := git.PlainOpen(gitExecutor.RepositoryPath)
  if err != nil {
    return nil, fmt.Errorf("failed to open repo: %w", err)
  }

  // Ensure the repository has a remote named "origin"
  err = ensureHasOrigin(repo)
  if err != nil {
    return nil, err
  }

  mainBranchName := "master"

  commits, err := GetCommitList(gitExecutor.RepositoryPath, mainBranchName, gitExecutor.gitPath)
  if err != nil {
    return nil, fmt.Errorf("failed to get commit list: %v", err)
  }

  if len(commits) == 0 {
    return make([]BranchResult, 0), nil
  }

  // map to keep all commits for each prefix
  prefixToCommits := make(map[string][]Commit)
  prefixPattern := regexp.MustCompile(`\[(.+?)]`)

  headCommit, err := repo.CommitObject(plumbing.NewHash(commits[0].Hash))
  if err != nil {
    return nil, fmt.Errorf("failed to get commit object: %v", err)
  }

  for _, commit := range commits {
    matches := prefixPattern.FindStringSubmatch(commit.Message)
    if len(matches) < 2 {
      continue
    }

    prefix := matches[1]
    prefixToCommits[prefix] = append(prefixToCommits[prefix], commit)
  }

  var results []BranchResult

  // for each prefix, create a branch with only the relevant commits
  for prefix, branchCommits := range prefixToCommits {
    currentParentHash := headCommit.ParentHashes[0]
    var lastCommitHash plumbing.Hash
    var commitDetails []CommitDetail

    isAnyCommitChanged := false

    // recreate each commit on top of the last one
    for _, originalCommit := range branchCommits {
      var detail CommitDetail
      detail, lastCommitHash, err = createOrUpdateCommit(originalCommit, currentParentHash, prefix, repo, gitExecutor)
      if err != nil {
        results = append(results, BranchResult{
          Name:  prefix,
          Error: err.Error(),
        })
        continue
      }
      commitDetails = append(commitDetails, detail)
      currentParentHash = lastCommitHash

      if detail.IsNew {
        isAnyCommitChanged = true
      }
    }

    // create the branch pointing to the last commit
    branchRefName := plumbing.NewBranchReferenceName(toFinalBranchName(branchPrefix, prefix))
    ref := plumbing.NewHashReference(branchRefName, lastCommitHash)
    branchSyncStatus := BranchCreated
    if _, err := repo.Reference(branchRefName, true); err == nil {
      if isAnyCommitChanged {
        branchSyncStatus = BranchUpdated
      } else {
        branchSyncStatus = BranchUnchanged
      }
    }
    if branchSyncStatus != BranchUnchanged {
      if err := repo.Storer.SetReference(ref); err != nil {
        results = append(results, BranchResult{
          Name:  prefix,
          Error: fmt.Sprintf("failed to set branch %s: %v", prefix, err),
        })
        continue
      }
    }

    results = append(results, BranchResult{
      Name:          prefix,
      SyncStatus:    branchSyncStatus,
      CommitCount:   len(commitDetails),
      CommitDetails: commitDetails,
    })
  }

  sort.Slice(results, func(i, j int) bool {
    return natural.Less(results[i].Name, results[j].Name)
  })
  return results, nil
}

func getCleanMessage(message, prefix string) string {
  if len(message) > len(prefix)+2 && message[0] == '[' {
    return message[len(prefix)+3:] // +3 for "[prefix] "
  }
  return message
}

func ensureHasOrigin(repo *git.Repository) error {
  if remotes, err := repo.Remotes(); err == nil {
    for _, remote := range remotes {
      if remote.Config().Name == "origin" {
        return nil
      }
    }
  }
  return fmt.Errorf("repository must have a remote named 'origin'")
}

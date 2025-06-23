package backend

import (
  "fmt"
  "regexp"
  "strings"

  "github.com/go-git/go-git/v5"
  "github.com/go-git/go-git/v5/plumbing"
)

func CreateBranches(repositoryPath string, branchPrefix string) ([]BranchResult, error) {
  branchPrefix = strings.TrimSuffix(branchPrefix, "/") + "/virtual/"

  // Open the Git repository
  repo, err := git.PlainOpen(repositoryPath)
  if err != nil {
    return nil, fmt.Errorf("failed to open repo: %w", err)
  }

  // Ensure the repository has a remote named "origin"
  err = ensureHasOrigin(repo)
  if err != nil {
    return nil, err
  }

  mainBranchName := "master"

  gitExecutor := NewGit(repositoryPath)

  commits, err := GetCommitList(repositoryPath, mainBranchName)
  if err != nil {
    return nil, fmt.Errorf("failed to get commit list: %v", err)
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

    // recreate each commit on top of the last one
    for _, originalCommit := range branchCommits {
      var detail CommitDetail
      detail, lastCommitHash, err = createOrUpdateBranch(originalCommit, currentParentHash, prefix, repo, gitExecutor)
      if err != nil {
        results = append(results, BranchResult{
          Name:  prefix,
          Error: err.Error(),
        })
        continue
      }
      commitDetails = append(commitDetails, detail)
      currentParentHash = lastCommitHash
    }

    // create the branch pointing to the last commit
    branchRefName := plumbing.NewBranchReferenceName(branchPrefix + prefix)
    ref := plumbing.NewHashReference(branchRefName, lastCommitHash)
    exists := false
    if _, err := repo.Reference(branchRefName, true); err == nil {
      exists = true
    }
    if err := repo.Storer.SetReference(ref); err != nil {
      results = append(results, BranchResult{
        Name:  prefix,
        Error: fmt.Sprintf("failed to set branch %s: %v", prefix, err),
      })
      continue
    }

    action := "Created"
    if exists {
      action = "Updated"
    }

    results = append(results, BranchResult{
      Name:          prefix,
      Action:        action,
      CommitCount:   len(commitDetails),
      CommitDetails: commitDetails,
    })
  }

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

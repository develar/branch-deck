package backend

import (
  "fmt"
  "strings"

  "github.com/go-git/go-git/v5"
  "github.com/go-git/go-git/v5/plumbing"
  "github.com/go-git/go-git/v5/plumbing/object"
)

//goland:noinspection GrazieInspection
func createOrUpdateBranch(
  originalCommit Commit,
  parentHash plumbing.Hash,
  prefix string,
  repo *git.Repository,
  git *Git,
) (CommitDetail, plumbing.Hash, error) {
  var isNewCommit bool

  // check if this commit was already cherry-picked using git notes
  cherryPickedCommitHash := plumbing.ZeroHash
  if originalCommit.Notes != "" {
    prefix := "v-commit:"
    if idx := strings.Index(originalCommit.Notes, prefix); idx != -1 {
      noteEnd := strings.Index(originalCommit.Notes[idx:], "\n")
      if noteEnd == -1 {
        noteEnd = len(originalCommit.Notes[idx:])
      }
      hashStr := strings.TrimSpace(originalCommit.Notes[idx+len(prefix) : idx+noteEnd])
      cherryPickedCommitHash = plumbing.NewHash(hashStr)
    }
  }

  // clean the message (remove prefix)
  message := getCleanMessage(originalCommit.Message, prefix)

  if !cherryPickedCommitHash.IsZero() {
    // reuse existing commit
    isNewCommit = false
  } else {
    var err error
    cherryPickedCommitHash, err = cherryPick(originalCommit, parentHash, repo, message, git)
    if err != nil {
      return CommitDetail{}, plumbing.ZeroHash, err
    }

    isNewCommit = true
  }

  return CommitDetail{
    Hash:    cherryPickedCommitHash.String()[:7],
    Message: message,
    IsNew:   isNewCommit,
  }, cherryPickedCommitHash, nil
}

func cherryPick(
  originalCommit Commit,
  parentHash plumbing.Hash,
  repo *git.Repository,
  message string,
  git *Git,
) (plumbing.Hash, error) {
  originalCommitObject, err := repo.CommitObject(plumbing.NewHash(originalCommit.Hash))
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to get commit object: %v", err)
  }

  // create new commit
  originalTree, err := originalCommitObject.Tree()
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to get tree for commit %s: %v", originalCommit.Hash, err)
  }

  newCommit := &object.Commit{
    Author:       originalCommitObject.Author,
    Message:      message,
    TreeHash:     originalTree.Hash,
    ParentHashes: []plumbing.Hash{parentHash},
  }

  obj := repo.Storer.NewEncodedObject()
  if err := newCommit.Encode(obj); err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to encode new commit: %v", err)
  }

  newCommitHash, err := repo.Storer.SetEncodedObject(obj)
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to save new commit: %v", err)
  }

  // store the mapping in git notes
  err = git.AddNote("v-commit:"+newCommitHash.String(), originalCommit.Hash)
  if err != nil {
    return plumbing.ZeroHash, err
  }
  return newCommitHash, nil
}

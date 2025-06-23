package backend

import (
  "fmt"
  "io"
  "log"
  "strings"

  "github.com/go-git/go-git/v5"
  "github.com/go-git/go-git/v5/plumbing"
)

func getCommitFromNotes(repo *git.Repository, originalHash, expectedParent plumbing.Hash) (plumbing.Hash, error) {
  // use git notes ref specific to cherry-picking with parent context
  notesRef := plumbing.ReferenceName("refs/notes/cherry-pick-mapping")

  // the note key combines the original hash and expected parent to handle different contexts
  noteKey := fmt.Sprintf("%s:%s", originalHash.String(), expectedParent.String())
  noteKeyHash := plumbing.ComputeHash(plumbing.BlobObject, []byte(noteKey))

  // try to get the note tree
  notesCommitRef, err := repo.Reference(notesRef, true)
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to get notes reference: %w", err)
  }

  notesCommit, err := repo.CommitObject(notesCommitRef.Hash())
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to get notes commit: %w", err)
  }

  notesTree, err := notesCommit.Tree()
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to get notes tree: %w", err)
  }

  // look for the note entry
  noteEntry, err := notesTree.FindEntry(noteKeyHash.String())
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to find note entry: %w", err)
  }

  // get the note blob
  noteBlob, err := repo.BlobObject(noteEntry.Hash)
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to get note blob: %w", err)
  }

  reader, err := noteBlob.Reader()
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to get blob reader: %w", err)
  }
  defer closeOrLog(reader)

  content, err := io.ReadAll(reader)
  if err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to read blob content: %w", err)
  }

  cherryPickedHash := plumbing.NewHash(strings.TrimSpace(string(content)))

  // verify the commit still exists
  if _, err := repo.CommitObject(cherryPickedHash); err != nil {
    return plumbing.ZeroHash, fmt.Errorf("failed to verify the cherry-picked commit exists: %w", err)
  }

  return cherryPickedHash, nil
}

//goland:noinspection GrazieInspection
func closeOrLog(c io.Closer) {
  if err := c.Close(); err != nil {
    log.Printf("Error while closing resource: %+v", err)
  }
}

package backend

import "testing"

func TestCreateBranches(t *testing.T) {
  _, err := CreateBranches("develar/test/", NewGit("/Users/develar/projects/idea", "git"))
  if err != nil {
    t.Error(err)
  }
}

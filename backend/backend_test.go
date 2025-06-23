package backend

import "testing"

func TestCreateBranches(t *testing.T) {
  _, err := CreateBranches("/Users/develar/projects/idea-1", "develar/test/")
  if err != nil {
    t.Error(err)
  }
}

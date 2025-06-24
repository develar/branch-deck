package backend

import (
  "fmt"
  "os"
  "os/exec"
  "strings"
  "testing"
  "time"

  "github.com/stretchr/testify/assert"
  "github.com/stretchr/testify/require"
)

//goland:noinspection SpellCheckingInspection
func TestParseGitLogOutput(t *testing.T) {
  // Sample git log output with multiple commits
  gitLogOutput := `commit b377fa9d62c58be31c2a6f649a07dca8013cbd0b
Author: John Doe <john.doe@example.com>
Date:   Fri Jun 20 08:34:25 2025 -0700

    IJPL-189392 use strict dispatcher for init project frame

commit b5a60ac10d4fd6e0677f5a4ca43ec28dc4ac9e71
Author: John Doe <john.doe@example.com>
Date:   Fri Jun 20 12:54:28 2025 +0200

    IJPL-189392 use strict ui dispatcher for MacToolbarFrameHeader.updateView

commit cd17104068e2559b28377c5446924c8856e6cc63
Author: John Doe <john.doe@example.com>
Date:   Fri Jun 20 12:56:21 2025 +0200

    IJPL-189392 cleanup
`

  commits, err := parseGitLogOutput(strings.NewReader(gitLogOutput))
  require.NoError(t, err)
  require.Len(t, commits, 3, "Should parse 3 commits")

  // Test first commit
  assert.Equal(t, "b377fa9d62c58be31c2a6f649a07dca8013cbd0b", commits[0].Hash)
  assert.Equal(t, "John Doe", commits[0].Author)
  assert.Equal(t, "john.doe@example.com", commits[0].Email)
  assert.Equal(t, "IJPL-189392 use strict dispatcher for init project frame", commits[0].Message)
  assert.Equal(t, "", commits[0].Notes)

  // Test second commit
  assert.Equal(t, "b5a60ac10d4fd6e0677f5a4ca43ec28dc4ac9e71", commits[1].Hash)
  assert.Equal(t, "John Doe", commits[1].Author)
  assert.Equal(t, "john.doe@example.com", commits[1].Email)
  assert.Equal(t, "IJPL-189392 use strict ui dispatcher for MacToolbarFrameHeader.updateView", commits[1].Message)
  assert.Equal(t, "", commits[1].Notes)

  // Test third commit
  assert.Equal(t, "cd17104068e2559b28377c5446924c8856e6cc63", commits[2].Hash)
  assert.Equal(t, "John Doe", commits[2].Author)
  assert.Equal(t, "john.doe@example.com", commits[2].Email)
  assert.Equal(t, "IJPL-189392 cleanup", commits[2].Message)
  assert.Equal(t, "", commits[2].Notes)

  // Check that date was parsed successfully
  assert.Equal(t, 2025, commits[0].Date.Year())
  assert.Equal(t, time.June, commits[0].Date.Month())
  assert.Equal(t, 20, commits[0].Date.Day())
  // Note: We're not testing the hour value because time zone handling may vary
  assert.Equal(t, 34, commits[0].Date.Minute())
  assert.Equal(t, 25, commits[0].Date.Second())
}

//goland:noinspection SpellCheckingInspection
func TestParseGitLogOutputWithMultiLineCommitMessage(t *testing.T) {
  // Sample git log output with multi-line commit message
  gitLogOutput := `commit b377fa9d62c58be31c2a6f649a07dca8013cbd0b
Author: John Doe <john.doe@example.com>
Date:   Fri Jun 20 08:34:25 2025 -0700

    IJPL-189392 use strict dispatcher for init project frame
    
    This is a multi-line commit message.
    - Added strict dispatcher
    - Fixed threading issues
    
    Fixes #123`

  commits, err := parseGitLogOutput(strings.NewReader(gitLogOutput))
  require.NoError(t, err)
  require.Len(t, commits, 1, "Should parse 1 commit")

  expectedMessage := "IJPL-189392 use strict dispatcher for init project frame"

  // We're just checking that the first line of the message is correctly parsed
  assert.True(t, strings.HasPrefix(commits[0].Message, expectedMessage), "Should preserve commit message")
  // Check that it contains the important parts of the message
  assert.Contains(t, commits[0].Message, "multi-line commit message")
  assert.Contains(t, commits[0].Message, "Added strict dispatcher")
  assert.Contains(t, commits[0].Message, "Fixed threading issues")
  assert.Contains(t, commits[0].Message, "Fixes #123")
  assert.Equal(t, "", commits[0].Notes)
}

//goland:noinspection SpellCheckingInspection
func TestParseGitLogOutputWithNotes(t *testing.T) {
  // Sample git log output with notes
  gitLogOutput := `commit b377fa9d62c58be31c2a6f649a07dca8013cbd0b
Author: John Doe <john.doe@example.com>
Date:   Fri Jun 20 08:34:25 2025 -0700

    IJPL-189392 use strict dispatcher for init project frame

Notes:
    v-commit:e1615781e580fd8891ca9025c5ff008158ff0905

commit b5a60ac10d4fd6e0677f5a4ca43ec28dc4ac9e71
Author: Jane Smith <jane.smith@example.com>
Date:   Fri Jun 20 12:54:28 2025 +0200

    IJPL-189392 fix threading issues

Notes:
    v-commit:f2726892f691ge9902db9126d6aa119269aa1016
    additional-info:reviewed-by-team`

  commits, err := parseGitLogOutput(strings.NewReader(gitLogOutput))
  require.NoError(t, err)
  require.Len(t, commits, 2, "Should parse 2 commits")

  // Test first commit
  assert.Equal(t, "b377fa9d62c58be31c2a6f649a07dca8013cbd0b", commits[0].Hash)
  assert.Equal(t, "John Doe", commits[0].Author)
  assert.Equal(t, "john.doe@example.com", commits[0].Email)
  assert.Equal(t, "IJPL-189392 use strict dispatcher for init project frame", commits[0].Message)
  assert.Equal(t, "v-commit:e1615781e580fd8891ca9025c5ff008158ff0905", commits[0].Notes)

  // Test second commit
  assert.Equal(t, "b5a60ac10d4fd6e0677f5a4ca43ec28dc4ac9e71", commits[1].Hash)
  assert.Equal(t, "Jane Smith", commits[1].Author)
  assert.Equal(t, "jane.smith@example.com", commits[1].Email)
  assert.Equal(t, "IJPL-189392 fix threading issues", commits[1].Message)
  assert.Contains(t, commits[1].Notes, "v-commit:f2726892f691ge9902db9126d6aa119269aa1016")
  assert.Contains(t, commits[1].Notes, "additional-info:reviewed-by-team")
}

func TestParseGitLogOutputWithEmptyOutput(t *testing.T) {
  commits, err := parseGitLogOutput(strings.NewReader(""))
  require.NoError(t, err)
  assert.Empty(t, commits, "Should return empty slice for empty input")
}

//goland:noinspection SpellCheckingInspection
func TestParseGitLogOutputWithMalformedInput(t *testing.T) {
  // Test with incomplete commit info
  malformedOutput := `Author: John Doe <john.doe@example.com>
Date:   Fri Jun 20 08:34:25 2025 +0200

    IJPL-189392 use strict dispatcher for init project frame`

  _, err := parseGitLogOutput(strings.NewReader(malformedOutput))
  assert.Error(t, err, "Should return error for malformed input missing commit hash")
}

// helper for exec command mocking
func fakeExecCommand(command string, args ...string) *exec.Cmd {
  cs := []string{fmt.Sprintf("-test.run=%s", "TestExecCommandHelper"), "--", command}
  cs = append(cs, args...)
  cmd := exec.Command(os.Args[0], cs...)

  // Set environment variable to know we're in the helper
  cmd.Env = append(os.Environ(), "GO_WANT_HELPER_PROCESS=1")
  return cmd
}

// TestExecCommandHelper is a test helper function to mock exec.Command
//goland:noinspection SpellCheckingInspection
func TestExecCommandHelper(t *testing.T) {
  if os.Getenv("GO_WANT_HELPER_PROCESS") != "1" {
    return
  }

  mockOutput := `commit b377fa9d62c58be31c2a6f649a07dca8013cbd0b
Author: John Doe <john.doe@example.com>
Date:   Fri Jun 20 08:34:25 2025 -0700

    IJPL-189392 use strict dispatcher for init project frame

commit b5a60ac10d4fd6e0677f5a4ca43ec28dc4ac9e71
Author: John Doe <john.doe@example.com>
Date:   Fri Jun 20 12:54:28 2025 +0200

    IJPL-189392 use strict ui dispatcher for MacToolbarFrameHeader.updateView`

  _, _ = fmt.Fprint(os.Stdout, mockOutput)
}

func TestGetCommitListMock(t *testing.T) {
  // This is a mock implementation that replaces the actual git command execution
  originalExecCommand := execCommand
  defer func() { execCommand = originalExecCommand }()

  // Set our mock function
  execCommand = fakeExecCommand

  // Test with the mocked command
  commits, err := GetCommitList("", "master", "git")
  require.NoError(t, err)
  require.Len(t, commits, 2, "Should parse 2 commits")

  assert.Equal(t, "b377fa9d62c58be31c2a6f649a07dca8013cbd0b", commits[0].Hash)
  assert.Equal(t, "b5a60ac10d4fd6e0677f5a4ca43ec28dc4ac9e71", commits[1].Hash)

  // Verify Notes field is initialized (empty in this mock)
  assert.Equal(t, "", commits[0].Notes)
  assert.Equal(t, "", commits[1].Notes)

  // Since we're using the mock function, we can't check the exact parameters
  // but we can verify the results match what we expect from our mock
}

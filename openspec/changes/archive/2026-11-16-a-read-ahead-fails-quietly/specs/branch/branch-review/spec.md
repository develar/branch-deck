## ADDED Requirements

### Requirement: Opening: A read ahead that fails is a warning

The read ahead is best effort (`readAhead` of src/lib/command.ts): its failure, of the command or of the IPC, MUST be a
warning of the log, never an error line or a rejection of the page, as the read of the window tells its own; the next
open MUST read it ahead again.

#### Scenario: A read ahead that fails
- **WHEN** the read ahead fails, the command or the IPC (a request WebKit fails as its page goes)
- **THEN** the log has a warning "Cannot read the review of a branch ahead", no error line, and the page no unhandled rejection
- **AND** the next open of the same state reads it ahead again, and one that read is not read again
  [@test] src/lib/review-window.test.ts (`is a warning where the IPC fails, no rejection of the page, and the next open reads ahead again; one that read is not read again`)
  [@test] src/lib/command.test.ts (`answers whether it read; a failure of the command or of the IPC is a warning, never an error or a rejection`)

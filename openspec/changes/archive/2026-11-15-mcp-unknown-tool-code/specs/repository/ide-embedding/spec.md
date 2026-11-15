## ADDED Requirements

### Requirement: Tools: A tool brd has not is refused with the code of the contract

A `tools/call` of a name that is none of the tools of `tools/list` MUST get `-32601 {reason: "unknownTool"}` and run
nothing. Every refusal of brd MUST have the code that `ErrorObject` of the contract gives its reason, and a reason it
gives no code the code of JSON-RPC (`badParams` and `unknownAction` `-32602`, `parse` `-32700`).

#### Scenario: A tool brd has not
- **WHEN** air-ide calls a tool that brd has not
- **THEN** brd answers `-32601 {reason: "unknownTool"}` and runs no command
- **AND** an unknown method, a call of an agent, an intent of the mirror, a missing resource, a subscription without a
  repository and a line that is no JSON get the code of their reason by the contract
  [@test] crates/mcp-host/src/server_test.rs (`every_refusal_of_brd_has_the_code_the_contract_gives_its_reason`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

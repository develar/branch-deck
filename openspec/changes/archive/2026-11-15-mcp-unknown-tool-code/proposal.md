# brd mcp answers an unknown tool with the code of the contract

## Why

`brd mcp` answered a `tools/call` of a tool it has not with `-32602 {reason: "unknownTool"}`, while the contract
(`ErrorObject` of `x-air-app.schema.json`) gives `unknownTool` the code `-32601`, as air-ide answers its own unknown
tools.

## What Changes

- An unknown tool gets `-32601 {reason: "unknownTool"}`.
- A test pins every refusal of brd to the code the contract gives its reason, read from `ErrorObject`; the other
  refusals had the right codes already.
- The app link corpus ends with a call of an unknown tool.
- Capability `repository/ide-embedding`.

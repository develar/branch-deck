# Tasks

- [x] 1.1 The instance file of `discovery.rs` has `schemaVersion`, `pid`, `product`, `build` and `socket` only.
- [x] 1.2 `Client::hosts` sends `{"v":1,"method":"hosts"}` and reads the hosts, `focusedAt` and a refusal.
- [x] 1.3 `ask_hosts` asks the running IDEs at once, each within `HOSTS_TIMEOUT`, and skips one that does not answer.
- [x] 1.4 `pick` takes the hosts of the answers: the deepest host, then the newest `focusedAt`, then the newest file.
- [x] 1.5 The tests of the client and of the discovery, and the spec `branch/start-in-air`.

## 1. The agent

- [x] 1.1 `Setup::bind` and `--bind <address>` of the `LaunchAgent`, which `PlistValues` reads back for an upgrade
- [x] 1.2 `put_binary`: a copy of this build at `Home::local_binary_file`, which `Setup::binary` names to the
  `LaunchAgent`
- [x] 1.3 `serve --bind` is a visible flag

## 2. The client

- [x] 2.1 `Client::setup_local` with the host file of the loopback address and the hello of `setup <alias>`
- [x] 2.2 `brd-host setup --local [<alias>]`

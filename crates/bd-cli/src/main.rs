//! `bd` as a console binary of its own, for Windows: the app binary is a GUI-subsystem program there, without a console
//! for the JSON output. macOS and Linux run the CLI from the app binary (`branch-deck cli`, or symlinked as `bd`).

fn main() {
  std::process::exit(bd_cli::run(std::env::args_os().skip(1)));
}

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use test_utils::test_repo_generator::TestRepoGenerator;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generate test Git repository with Branch Deck conflict scenarios", long_about = None)]
struct Args {
  /// Output directory for the test repository
  #[arg(short, long, default_value = "./test-repo")]
  output: PathBuf,
}

fn main() -> Result<()> {
  let args = Args::parse();

  println!("🚀 Generating test repository with Branch Deck conflicts...");
  println!("📁 Output directory: {}", args.output.display());

  // Create generator
  let generator = TestRepoGenerator::new();

  // Generate repository with conflicts
  generator.generate(&args.output)?;

  let working_dir = args.output.join("working");
  println!("\n✨ Repository structure created:");
  println!("   📁 {}/", args.output.display());
  println!("   ├── 📦 origin.git/    (bare repository - the remote)");
  println!("   └── 💻 working/       (working repository - open this in Branch Deck)");

  println!("\n💡 Next steps:");
  println!("   cd {}", working_dir.display());
  println!("   git log --oneline --graph --all");
  println!("\n🎯 To test in Branch Deck:");
  println!("   Open: {}", working_dir.display());

  Ok(())
}

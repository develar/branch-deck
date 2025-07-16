use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use test_utils::TestRepoGenerator;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generate test Git repositories with realistic commit patterns", long_about = None)]
struct Args {
  /// Output directory for the test repository
  #[arg(short, long, default_value = "./test-repo")]
  output: PathBuf,

  /// Random seed for reproducible generation
  #[arg(short, long)]
  seed: Option<u64>,

  /// Preset configuration to use
  #[arg(short, long, value_enum, default_value = "standard")]
  preset: Preset,

  /// Show detailed statistics after generation
  #[arg(short = 'v', long)]
  verbose: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Preset {
  /// Standard test repository with mixed commits
  Standard,
  /// Repository with many conflict scenarios
  Conflicts,
  /// Repository focused on missing commit scenarios
  MissingCommits,
  /// Large repository with many commits
  Large,
}

fn main() -> Result<()> {
  let args = Args::parse();

  println!("🚀 Generating test repository...");
  println!("📁 Output directory: {}", args.output.display());

  if let Some(seed) = args.seed {
    println!("🎲 Using seed: {seed}");
  }

  println!("📋 Preset: {:?}", args.preset);

  // Create generator
  let mut generator = if let Some(seed) = args.seed {
    TestRepoGenerator::with_seed(seed)
  } else {
    TestRepoGenerator::new()
  };

  // Generate based on preset
  let stats = match args.preset {
    Preset::Standard => generator.generate(&args.output)?,
    Preset::Conflicts => generator.generate_with_conflicts(&args.output)?,
    Preset::MissingCommits => generator.generate_with_missing_commits(&args.output)?,
    Preset::Large => generator.generate_large(&args.output)?,
  };

  println!("✅ Test repository generated successfully!");

  if args.verbose {
    println!("\n📊 Repository Statistics:");
    println!("   - Issue commits: {}", stats.issue_commits);
    println!("   - Maintenance commits: {}", stats.maintenance_commits);
    println!("   - Conflict branches: {}", stats.conflict_branches);
    println!("   - Total commits: {}", stats.total_commits());
  }

  println!("\n💡 Next steps:");
  println!("   cd {}", args.output.display());
  println!("   git log --oneline --graph --all");

  Ok(())
}

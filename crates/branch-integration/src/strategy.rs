/// Detection strategy configuration
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DetectionStrategy {
  /// Default - fast rebase/cherry-pick detection only
  #[default]
  Rebase,
  /// Include merge commit detection
  Merge,
  /// Include expensive squash merge detection
  Squash,
  /// Run all available detection methods (for comprehensive testing)
  All,
}

/// Get the detection strategy based on runtime configuration
pub fn get_detection_strategy() -> DetectionStrategy {
  if std::env::var("BRANCH_DECK_FULL_DETECTION").is_ok() {
    DetectionStrategy::All
  } else {
    DetectionStrategy::Rebase
  }
}

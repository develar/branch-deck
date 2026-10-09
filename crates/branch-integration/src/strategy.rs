use std::sync::LazyLock;

/// Detection strategy configuration
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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

/// Get the detection strategy based on runtime configuration.
/// The environment is read once per process, not on every sync.
pub fn get_detection_strategy() -> DetectionStrategy {
  static STRATEGY: LazyLock<DetectionStrategy> = LazyLock::new(|| {
    if std::env::var_os("BRANCH_DECK_FULL_DETECTION").is_some() {
      DetectionStrategy::All
    } else {
      DetectionStrategy::Rebase
    }
  });
  *STRATEGY
}

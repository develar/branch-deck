/// Common constants used across all model implementations
/// Random seed for LogitsProcessor - using speed of light in m/s for reproducibility
pub const LOGITS_PROCESSOR_SEED: u64 = 299792458;

/// Buffer to reserve for generation when checking context limits
pub const GENERATION_BUFFER: usize = 200;

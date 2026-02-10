//! Noise suppression and audio cleanup.
//!
//! Integrates RNNoise for real-time noise suppression.

use grabme_common::error::{GrabmeError, GrabmeResult};

/// Noise suppression configuration.
#[derive(Debug, Clone)]
pub struct NoiseSuppressionConfig {
    /// Suppression strength [0.0, 1.0].
    pub strength: f64,

    /// Whether to apply a noise gate.
    pub noise_gate: bool,

    /// Noise gate threshold in dB.
    pub gate_threshold_db: f64,
}

impl Default for NoiseSuppressionConfig {
    fn default() -> Self {
        Self {
            strength: 0.8,
            noise_gate: true,
            gate_threshold_db: -40.0,
        }
    }
}

/// Apply noise suppression to an audio file.
pub fn suppress_noise(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    config: &NoiseSuppressionConfig,
) -> GrabmeResult<()> {
    tracing::info!(
        input = %input_path.display(),
        output = %output_path.display(),
        strength = config.strength,
        "Applying noise suppression"
    );

    if !input_path.exists() {
        return Err(GrabmeError::FileNotFound {
            path: input_path.to_path_buf(),
        });
    }

    // TODO: Phase 5 implementation:
    // 1. Load audio file
    // 2. Initialize RNNoise
    // 3. Process frames through noise suppression
    // 4. Apply noise gate if configured
    // 5. Write output file

    Err(GrabmeError::unsupported(
        "Noise suppression will be implemented in Phase 5 (rnnoise integration)",
    ))
}

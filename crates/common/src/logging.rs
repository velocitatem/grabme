//! Logging and tracing initialization.

use crate::config::LoggingConfig;

/// Initialize the tracing subscriber with the given configuration.
pub fn init_logging(config: &LoggingConfig) {
    use tracing_subscriber::{fmt, EnvFilter};

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));

    if config.json {
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(env_filter)
            .json()
            .finish();
        tracing::subscriber::set_global_default(subscriber).ok();
    } else {
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber).ok();
    }
}

/// Initialize logging with defaults (useful for tests and quick scripts).
pub fn init_default_logging() {
    init_logging(&LoggingConfig::default());
}

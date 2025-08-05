//! loggingfunctionmodular

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initializeloggingsystem
pub fn init(level: &str) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}

/// Setlogginglevel
pub fn set_level(level: &str) {
    tracing::info!("Log level set to: {}", level);
}

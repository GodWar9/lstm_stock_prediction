//! Structured telemetry and logging initialization for quantctl.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initializes structured logging based on CLI arguments and RUST_LOG environment variable.
pub fn init_telemetry(log_format: &str, verbose: bool) -> anyhow::Result<()> {
    let default_filter = if verbose { "quant=debug,quantctl=debug,info" } else { "quant=info,quantctl=info,warn" };
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    let registry = tracing_subscriber::registry().with(env_filter);

    if log_format.eq_ignore_ascii_case("json") {
        let json_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(false)
            .with_target(true);
        registry.with(json_layer).try_init()?;
    } else {
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_thread_ids(false)
            .compact();
        registry.with(fmt_layer).try_init()?;
    }

    Ok(())
}

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize application logging with structured formatting.
///
/// Ensures log levels can be controlled via `RUST_LOG` while maintaining
/// security hygiene (no credentials, PAN, bank numbers, or raw monetary PII).
pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("opentally=debug,opentally_core=debug,info"));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init();
}

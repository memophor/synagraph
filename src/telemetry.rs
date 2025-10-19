// SynaGraph is open-source under the Apache License 2.0; see LICENSE for usage and contributions.
// Telemetry helpers set up opinionated tracing defaults for local and production deployments.

use tracing_subscriber::{fmt, EnvFilter};

pub fn init() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("synagraph=info,tower_http=info"));

    if tracing::subscriber::set_global_default(
        fmt::Subscriber::builder()
            .with_env_filter(env_filter)
            .compact()
            .finish(),
    )
    .is_err()
    {
        // Default subscriber already installed; this is fine in tests.
    }
}

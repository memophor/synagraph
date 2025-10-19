use tracing_subscriber::{fmt, EnvFilter};

pub fn init() {
    if tracing::subscriber::set_global_default(
        fmt::Subscriber::builder()
            .with_env_filter(EnvFilter::from_default_env())
            .compact()
            .finish(),
    )
    .is_err()
    {
        // Default subscriber already installed; this is fine in tests.
    }
}

use std::path::PathBuf;

pub fn log_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("inferno_aoip")
        .join("logs")
}

pub fn init_rolling_logs() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = log_dir();
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = tracing_appender::rolling::daily(&log_dir, "inferno.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive(tracing::Level::DEBUG.into())
                )
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive(tracing::Level::INFO.into())
                )
        )
        .init();
    
    guard
}

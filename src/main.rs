use rust_starterkit::{AppState, app, config::Config, db, migration::Migrator};
use sea_orm_migration::MigratorTrait;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            tracing::error!(error = %message, "application stopped");
            ExitCode::FAILURE
        }
    }
}
async fn run() -> Result<(), String> {
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    if !matches!(command.as_str(), "serve" | "migrate") {
        return Err("usage: rust-starterkit [serve|migrate]".into());
    }
    let config = Config::from_env()?;
    let database = db::connect(&config)
        .await
        .map_err(|_| "database connection failed")?;
    if command == "migrate" {
        Migrator::up(&database, None)
            .await
            .map_err(|_| "migration failed; inspect migration status with an administrator")?;
        database
            .close()
            .await
            .map_err(|_| "database close failed")?;
        return Ok(());
    }
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|_| "HTTP bind failed")?;
    tracing::info!(bind = %config.bind, example_enabled = config.enable_example, "listening");
    axum::serve(
        listener,
        app(
            AppState {
                db: database.clone(),
            },
            &config,
        ),
    )
    .with_graceful_shutdown(shutdown())
    .await
    .map_err(|_| "HTTP server failed")?;
    database
        .close()
        .await
        .map_err(|_| "database close failed")?;
    Ok(())
}
async fn shutdown() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl-C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    tracing::info!("shutdown requested; draining requests");
}

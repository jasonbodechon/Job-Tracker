use std::time::Duration;
use job_tracker_web::{config::Config, service, AppState};
#[tokio::main] async fn main(){tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();let config=Config::from_env().expect("valid worker configuration");let poll=config.worker_poll_seconds;let state=AppState::connect(config).await.expect("database connection and migrations");loop{if let Err(error)=service::worker_tick(&state).await{tracing::error!(%error,"worker tick failed");}tokio::time::sleep(Duration::from_secs(poll)).await;}}

pub mod api;
pub mod app;
pub mod docs;
pub mod dtos;
pub mod extractors;
pub mod router;
pub mod services;

/*********************************************
 *
 *
 *
 ********************************************/

// use anyhow::{Context, Result};
// use app::ApplicationServer;
// use clap::Parser;
// use dotenvy::dotenv;
// use std::sync::Arc;
// use utils::AppConfig;
//
// #[tokio::main]
// async fn main() -> Result<(), anyhow::Error> {
//     dotenv().ok();
//
//     let config = Arc::new(AppConfig::parse());
//
//     ApplicationServer::serve(config)
//         .await
//         .context("🔴 Failed to start server")?;
//
//     Ok(())
// }

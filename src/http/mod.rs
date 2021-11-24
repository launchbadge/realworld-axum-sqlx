use crate::config::Config;
use anyhow::Context;
use axum::{AddExtensionLayer, Router};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceBuilder;

mod error;
mod extractor;
mod users;

use error::Error;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone)]
struct ApiContext {
    config: Arc<Config>,
    db: PgPool,
}

pub async fn serve(config: Config, db: PgPool) -> anyhow::Result<()> {
    let app = api_routes().layer(
        ServiceBuilder::new().layer(AddExtensionLayer::new(ApiContext {
            config: Arc::new(config),
            db,
        })),
    );

    axum::Server::bind(&"0.0.0.0:8080".parse()?)
        .serve(app.into_make_service())
        .await
        .context("error running HTTP server")
}

fn api_routes() -> Router {
    users::router()
}

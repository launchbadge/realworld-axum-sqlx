use crate::config::Config;
use anyhow::Context;
use axum::Router;
use sqlx::PgPool;
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::net::TcpListener;

// Utility modules.

/// Defines a common error type to use for all request handlers, compliant with the Realworld spec.
mod error;

/// Contains definitions for application-specific parameters to handler functions,
/// such as `AuthUser` which checks for the `Authorization: Token <token>` header in the request,
/// verifies `<token>` as a JWT and checks the signature,
/// then deserializes the information it contains.
mod extractor;

/// A catch-all module for other common types in the API. Arguably, the `error` and `extractor`
/// modules could have been children of this one, but that's more of a subjective decision.
mod types;

// Modules introducing API routes. The names match the routes listed in the Realworld spec,
// although the `articles` module also includes the `GET /api/tags` route because it touches
// the `article` table.
//
// This is not the order they were written in; `rustfmt` auto-sorts them.
// However, you should follow the order they were written in because some of the comments
// are more stream-of-consciousness and assume you read them in a particular order.
//
// See `api_router()` below for the recommended order.
mod articles;
mod profiles;
mod users;

pub use error::{Error, ResultExt};

pub type Result<T, E = Error> = std::result::Result<T, E>;

use tower_http::trace::TraceLayer;

/// The core type through which handler functions can access common API state.
///
/// This can be accessed by adding a parameter `State<ApiContext>` to a handler function's
/// parameters.
///
/// In other projects I've passed this stuff as separate objects, e.g.
/// using a separate actix-web `Data` extractor for each of `Config`, `PgPool`, etc.
/// It just ends up being kind of annoying that way, but does have the whole
/// "pass only what you need where you need it" angle.
///
/// It may not be a bad idea if you need your API to be more modular (turn routes
/// on and off, and disable any unused extension objects) but it's really up to a
/// judgement call.
#[derive(Clone)]
pub(crate) struct ApiContext {
    config: Arc<Config>,
    db: PgPool,
}

pub async fn serve(config: Config, db: PgPool) -> anyhow::Result<()> {
    let api_context = ApiContext {
        config: Arc::new(config),
        db,
    };

    // Bootstrapping an API is both more intuitive with Axum than Actix-web but also
    // a bit more confusing at the same time.
    //
    // Coming from Actix-web, I would expect to pass the router into `ServiceBuilder` and not
    // the other way around.
    //
    // It does look nicer than the mess of `move || {}` closures you have to do with Actix-web,
    // which, I suspect, largely has to do with how it manages its own worker threads instead of
    // letting Tokio do it.
    let app = api_router(api_context);

    // We use 8080 as our default HTTP server port, it's pretty easy to remember.
    //
    // Note that any port below 1024 needs superuser privileges to bind on Linux,
    // so 80 isn't usually used as a default for that reason.
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .await
        .context("error running HTTP server")
}

fn api_router(api_context: ApiContext) -> Router {
    // This is the order that the modules were authored in.
    Router::new()
        .merge(users::router())
        .merge(profiles::router())
        .merge(articles::router())
        // Enables logging. Use `RUST_LOG=tower_http=debug`
        .layer(TraceLayer::new_for_http())
        .with_state(api_context)
}

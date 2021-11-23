use axum::extract::Extension;
use axum::Router;
use axum::routing::post;
use crate::http::ApiContext;

pub fn router() -> Router {
    Router::new()
        .route("/api/users", post())
}

#[derive(serde::Deserialize)]
struct CreateUserRequest {
    user: NewUser,
}

async fn create_user(
    context: Extension<ApiContext>,

)
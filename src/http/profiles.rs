use crate::http::error::ResultExt;
use crate::http::extractor::{AuthUser, MaybeAuthUser};
use crate::http::ApiContext;
use crate::http::{Error, Result};
use axum::extract::{Extension, Path};
use axum::routing::{get, post};
use axum::{Json, Router};

// The `profiles` routes are very similar to the `users` routes, except they allow looking up
// other users' data.

pub fn router() -> Router {
    Router::new()
        .route("/api/profiles/:username", get(get_user_profile))
        .route(
            "/api/profiles/:username/follow",
            post(follow_user).delete(unfollow_user),
        )
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/api-response-format#profile
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileBody {
    profile: Profile,
}

#[derive(serde::Serialize)]
pub struct Profile {
    pub username: String,
    pub bio: String,
    pub image: Option<String>,
    pub following: bool,
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-profile
async fn get_user_profile(
    // The Realworld spec says authentication is optional, but doesn't specify if it should be
    // validated if the `Authorization` header is present. I've chosen to do so.
    //
    // See the docs for `MaybeAuthUser` for why this isn't just `Option<AuthUser>`.
    maybe_auth_user: MaybeAuthUser,
    ctx: Extension<ApiContext>,
    // Destructuring `Path()` is something I've missed in Actix-web since it was removed
    // in the 4.0 beta: https://github.com/actix/actix-web/pull/2160
    // Needless to say, I'm delighted that Axum has it.
    Path(username): Path<String>,
) -> Result<Json<ProfileBody>> {
    // Since our query columns directly match an existing struct definition,
    // we can use `query_as!()` and save a bit of manual mapping.
    let profile = sqlx::query_as!(
        Profile,
        r#"
            select
                username,
                bio,
                image,
                exists(
                    select 1 from follow 
                    where followed_user_id = "user".user_id and following_user_id = $2
                ) "following!" -- This tells SQLx that this column will never be null
            from "user"
            where username = $1
        "#,
        username,
        maybe_auth_user.user_id()
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or(Error::NotFound)?;

    Ok(Json(ProfileBody { profile }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#follow-user
async fn follow_user(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Path(username): Path<String>,
) -> Result<Json<ProfileBody>> {
    // You can implement this either with a single query using Common Table Expressions (CTEs),
    // or multiple queries with a transaction.
    //
    // The former is likely more performant as it involves only a single round-trip to the database,
    // but the latter is more readable.
    //
    // It's generally a good idea to shoot for readability over raw performance for long-lived
    // projects. You don't want to come back later and be unable to understand what you wrote
    // because you were too clever. You can always improve performance later if the
    // implementation proves to be a bottleneck.
    //
    // Readability is also paramount if you need to onboard more devs to the project.
    //
    // Trust me, I've learned this the hard way.

    // Begin a transaction so we have a consistent view of the database.
    // This has the side-effect of checking out a connection for the whole function,
    // which saves some overhead on subsequent queries.
    //
    // If an error occurs, this transaction will be rolled back on-drop.
    let mut tx = ctx.db.begin().await?;

    let user = sqlx::query!(
        r#"select user_id, username, bio, image from "user" where username = $1"#,
        username
    )
    .fetch_optional(&mut tx)
    .await?
    .ok_or(Error::NotFound)?;

    sqlx::query!(
        "insert into follow(following_user_id, followed_user_id) values ($1, $2) \
         on conflict do nothing", // If the row already exists, we don't need to do anything.
        auth_user.user_id,
        user.user_id
    )
    .execute(&mut tx)
    .await
    // Handle this check constraint
    .on_constraint("user_cannot_follow_self", |_| Error::Forbidden)?;

    // IMPORTANT! Without this, the changes we just made will be dropped.
    tx.commit().await?;

    Ok(Json(ProfileBody {
        profile: Profile {
            username: user.username,
            bio: user.bio,
            image: user.image,
            // We just made sure of this.
            following: true,
        },
    }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#unfollow-user
async fn unfollow_user(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Path(username): Path<String>,
) -> Result<Json<ProfileBody>> {
    // This is basically identical to `follow_user()` user except we're deleting from `follow`.

    let mut tx = ctx.db.begin().await?;

    let user = sqlx::query!(
        r#"select user_id, username, bio, image from "user" where username = $1"#,
        username
    )
    .fetch_optional(&mut tx)
    .await?
    .ok_or(Error::NotFound)?;

    sqlx::query!(
        "delete from follow where following_user_id = $1 and followed_user_id = $2",
        auth_user.user_id,
        user.user_id
    )
    .execute(&mut tx)
    .await?;

    // IMPORTANT! Without this, the changes we just made will be dropped.
    tx.commit().await?;

    Ok(Json(ProfileBody {
        profile: Profile {
            username: user.username,
            bio: user.bio,
            image: user.image,
            // We just made sure of this.
            following: false,
        },
    }))
}

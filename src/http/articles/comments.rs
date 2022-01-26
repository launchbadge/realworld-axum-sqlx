use crate::http::extractor::{AuthUser, MaybeAuthUser};
use crate::http::profiles::Profile;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use axum::extract::{Extension, Path};
use axum::routing::{delete, get};
use axum::{Json, Router};
use futures::TryStreamExt;
use time::OffsetDateTime;

pub fn router() -> Router {
    // Unlike those in `listing`, these routes are fortunately all self-contained
    Router::new()
        .route(
            "/api/articles/:slug/comments",
            get(get_article_comments).post(add_comment),
        )
        .route(
            "/api/articles/:slug/comments/:comment_id",
            delete(delete_comment),
        )
}

#[derive(serde::Deserialize, serde::Serialize)]
struct CommentBody<T = Comment> {
    comment: T,
}

#[derive(serde::Serialize)]
struct MultipleCommentsBody {
    comments: Vec<Comment>,
}

#[derive(serde::Deserialize)]
struct AddComment {
    body: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Comment {
    id: i64,
    created_at: Timestamptz,
    updated_at: Timestamptz,
    body: String,
    author: Profile,
}

// Same thing as `ArticleFromQuery`
struct CommentFromQuery {
    comment_id: i64,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    body: String,
    author_username: String,
    author_bio: String,
    author_image: Option<String>,
    following_author: bool,
}

impl CommentFromQuery {
    fn into_comment(self) -> Comment {
        Comment {
            id: self.comment_id,
            // doing this conversion in-code does save having to use the type overrides in query
            created_at: Timestamptz(self.created_at),
            updated_at: Timestamptz(self.updated_at),
            body: self.body,
            author: Profile {
                username: self.author_username,
                bio: self.author_bio,
                image: self.author_image,
                following: self.following_author,
            },
        }
    }
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-comments-from-an-article
async fn get_article_comments(
    maybe_auth_user: MaybeAuthUser,
    ctx: Extension<ApiContext>,
    Path(slug): Path<String>,
) -> Result<Json<MultipleCommentsBody>> {
    // With this, we can return 404 if the article slug was not found.
    let article_id = sqlx::query_scalar!("select article_id from article where slug = $1", slug)
        .fetch_optional(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;

    let comments = sqlx::query_as!(
        CommentFromQuery,
        r#"
            select
                comment_id,
                comment.created_at,
                comment.updated_at,
                comment.body,
                author.username author_username,
                author.bio author_bio,
                author.image author_image,
                exists(select 1 from follow where followed_user_id = author.user_id and following_user_id = $1) "following_author!"
            from article_comment comment
            inner join "user" author using (user_id)
            where article_id = $2
            order by created_at
        "#,
        maybe_auth_user.user_id(),
        article_id
    )
        .fetch(&ctx.db)
        .map_ok(CommentFromQuery::into_comment)
        .try_collect()
        .await?;

    Ok(Json(MultipleCommentsBody { comments }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#add-comments-to-an-article
async fn add_comment(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Path(slug): Path<String>,
    req: Json<CommentBody<AddComment>>,
) -> Result<Json<CommentBody>> {
    let comment = sqlx::query_as!(
        CommentFromQuery,
        r#"
            with inserted_comment as (
                insert into article_comment(article_id, user_id, body)
                select article_id, $1, $2
                from article
                where slug = $3
                returning comment_id, created_at, updated_at, body
            )
            select
                comment_id,
                comment.created_at,
                comment.updated_at,
                body,
                author.username author_username,
                author.bio author_bio,
                author.image author_image,
                false "following_author!"
            from inserted_comment comment
            inner join "user" author on user_id = $1
        "#,
        auth_user.user_id,
        req.comment.body,
        slug
    )
    .fetch_optional(&ctx.db)
    .await?
    // In this case, we know a comment should have been inserted unless the article slug
    // was not found.
    .ok_or(Error::NotFound)?
    .into_comment();

    Ok(Json(CommentBody { comment }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#delete-comment
async fn delete_comment(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Path((slug, comment_id)): Path<(String, i64)>,
) -> Result<()> {
    // Identical technique to `articles::delete_article()`
    let result = sqlx::query!(
        r#"
            with deleted_comment as (
                delete from article_comment
                where 
                    comment_id = $1
                    and article_id in (select article_id from article where slug = $2)
                    and user_id = $3
                returning 1 
            )
            select 
                exists(
                    select 1 from article_comment
                    inner join article using (article_id)
                    where comment_id = $1 and slug = $2
                ) "existed!",
                exists(select 1 from deleted_comment) "deleted!"
        "#,
        comment_id,
        slug,
        auth_user.user_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if result.deleted {
        Ok(())
    } else if result.existed {
        Err(Error::Forbidden)
    } else {
        Err(Error::NotFound)
    }
}

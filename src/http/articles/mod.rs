use axum::extract::{Extension, Path};
use axum::routing::{get, post};
use axum::{Json, Router};
use itertools::Itertools;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

use crate::http::extractor::{AuthUser, MaybeAuthUser};
use crate::http::profiles::Profile;
use crate::http::types::Timestamptz;
use crate::http::{ApiContext, Error, Result, ResultExt};

mod comments;
mod listing;

pub fn router() -> Router {
    // I would prefer `listing` to have its own `router()` method and keep the handler
    // functions private, however that doesn't really work here as we need to list all the
    // verbs under the same path exactly once.
    Router::new()
        .route(
            "/api/articles",
            post(create_article).get(listing::list_articles),
        )
        // `feed_articles` could be private technically, but meh
        .route("/api/articles/feed", get(listing::feed_articles))
        .route(
            "/api/articles/:slug",
            get(get_article).put(update_article).delete(delete_article),
        )
        .route(
            "/api/articles/:slug/favorite",
            post(favorite_article).delete(unfavorite_article),
        )
        // This route isn't technically grouped with articles but it makes sense to include it
        // here since it touches the `article` table.
        .route("/api/tags", get(get_tags))
        .merge(comments::router())
}

#[derive(serde::Deserialize, serde::Serialize)]
// Just trying this out to avoid the tautology of `ArticleBody<Article>`
struct ArticleBody<T = Article> {
    article: T,
}

#[derive(serde::Serialize)]
struct TagsBody {
    tags: Vec<String>,
}

#[derive(serde::Deserialize)]
// The Realworld spec doesn't mention this as an API convention, it just finally shows up
// when you're looking at the spec for the Article object and see `tagList` as a field name.
#[serde(rename_all = "camelCase")]
struct CreateArticle {
    title: String,
    description: String,
    body: String,
    tag_list: Vec<String>,
}

#[derive(serde::Deserialize)]
struct UpdateArticle {
    title: Option<String>,
    description: Option<String>,
    body: Option<String>,
    // Interestingly, the spec omits `tagList` from this route.
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Article {
    slug: String,
    title: String,
    description: String,
    body: String,
    tag_list: Vec<String>,
    created_at: Timestamptz,
    // Note: the Postman collection included with the spec assumes that this is never null.
    // We prefer to leave it unset unless the row has actually be updated.
    updated_at: Timestamptz,
    favorited: bool,
    favorites_count: i64,
    author: Profile,
}

// One place that SQLx could still improve upon is when a query wants to return a nested
// object, such as `Article` wants to with the `author` field.
// For 1:1 relations like that, what we usually do is deserialize the nested object as columns
// flattened into the main query, then fixup the structure afterwards.
//
// It's a good chunk of boilerplate but thankfully you usually only have to write it a few
// times across a whole project.
struct ArticleFromQuery {
    slug: String,
    title: String,
    description: String,
    body: String,
    tag_list: Vec<String>,
    created_at: Timestamptz,
    updated_at: Timestamptz,
    favorited: bool,
    favorites_count: i64,
    author_username: String,
    author_bio: String,
    author_image: Option<String>,
    // This was originally `author_following` to match other fields but that's kind of confusing.
    // That made it sound like a flag showing if the author is following the current user
    // but the intent is the other way round.
    following_author: bool,
}

impl ArticleFromQuery {
    fn into_article(self) -> Article {
        Article {
            slug: self.slug,
            title: self.title,
            description: self.description,
            body: self.body,
            tag_list: self.tag_list,
            created_at: self.created_at,
            updated_at: self.updated_at,
            favorited: self.favorited,
            favorites_count: self.favorites_count,
            author: Profile {
                username: self.author_username,
                bio: self.author_bio,
                image: self.author_image,
                following: self.following_author,
            },
        }
    }
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#create-article
async fn create_article(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Json(mut req): Json<ArticleBody<CreateArticle>>,
) -> Result<Json<ArticleBody>> {
    let slug = slugify(&req.article.title);

    // Never specified unless you count just showing them sorted in the examples:
    // https://realworld-docs.netlify.app/docs/specs/backend-specs/api-response-format#single-article
    //
    // However, it is required by the Postman collection. To their credit, the Realworld authors
    // have acknowledged this oversight and are willing to loosen the requirement:
    // https://github.com/gothinkster/realworld/issues/839#issuecomment-1002806224
    req.article.tag_list.sort();

    // For fun, this is how we combine several operations into a single query for brevity.
    let article = sqlx::query_as!(
        ArticleFromQuery,
        // language=PostgreSQL
        r#"
            with inserted_article as (
                insert into article (user_id, slug, title, description, body, tag_list)
                values ($1, $2, $3, $4, $5, $6)
                returning 
                    slug, 
                    title, 
                    description, 
                    body, 
                    tag_list, 
                    -- This is how you can override the inferred type of a column.
                    created_at "created_at: Timestamptz", 
                    updated_at "updated_at: Timestamptz"
            )
            select 
                inserted_article.*,
                false "favorited!",
                0::int8 "favorites_count!",
                username author_username,
                bio author_bio,
                image author_image,
                -- user is forbidden to follow themselves
                false "following_author!"
            from inserted_article
            inner join "user" on user_id = $1
        "#,
        auth_user.user_id,
        slug,
        req.article.title,
        req.article.description,
        req.article.body,
        // The typechecking code that SQLx emits for parameters sometimes chokes on vectors.
        // This slicing operation shouldn't be required, but it took a mess of type-system
        // hacks just to get the codegen this far.
        &req.article.tag_list[..]
    )
    .fetch_one(&ctx.db)
    .await
    .on_constraint("article_slug_key", |_| {
        Error::unprocessable_entity([("slug", format!("duplicate article slug: {}", slug))])
    })?;

    Ok(Json(ArticleBody {
        article: article.into_article(),
    }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#update-article
async fn update_article(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Path(slug): Path<String>,
    Json(req): Json<ArticleBody<UpdateArticle>>,
) -> Result<Json<ArticleBody>> {
    let mut tx = ctx.db.begin().await?;

    let new_slug = req.article.title.as_deref().map(slugify);

    let article_meta = sqlx::query!(
        // This locks the `article` row for the duration of the transaction so we're
        // not interleaving this with other possible updates.
        "select article_id, user_id from article where slug = $1 for update",
        slug
    )
    .fetch_optional(&mut tx)
    .await?
    .ok_or(Error::NotFound)?;

    if article_meta.user_id != auth_user.user_id {
        return Err(Error::Forbidden);
    }

    // Update the article and return the new values in the same query.
    //
    // This is perhaps toeing the line of "too clever" as I talked about in `profiles::follow_user()`,
    // however I think here it saves us some code duplication as well as an extra round-trip
    // to the database, and isn't too hard to understand.
    //
    // I could also have folded the above permission check into the update, and have in the past,
    // but I think that's where it starts to get too confusing as it relies on the fact that CTEs
    // with `INSERT/UPDATE/DELETE` statements are executed even if they're not read from.
    let article = sqlx::query_as!(
        ArticleFromQuery,
        // language=PostgreSQL
        r#"
            with updated_article as (
                update article
                set
                    slug = coalesce($1, slug),
                    title = coalesce($2, title),
                    description = coalesce($3, description),
                    body = coalesce($4, body)
                where article_id = $5
                returning
                    slug,
                    title,
                    description,
                    body,
                    tag_list,
                    article.created_at "created_at: Timestamptz",
                    article.updated_at "updated_at: Timestamptz"
            )
            select
                updated_article.*,
                exists(select 1 from article_favorite where user_id = $6) "favorited!",
                coalesce(
                    (select count(*) from article_favorite fav where fav.article_id = $5),
                    0
                ) "favorites_count!",
                author.username author_username,
                author.bio author_bio,
                author.image author_image,
                -- user not allowed to follow themselves
                false "following_author!"
            from updated_article
            -- we've ensured the current user is the article's author so we can assume it here
            inner join "user" author on author.user_id = $6
        "#,
        // In a query with a lot of bind parameters, it can be difficult to keep them all straight.
        // There's an open proposal to improve this: https://github.com/launchbadge/sqlx/issues/875
        new_slug,
        req.article.title,
        req.article.description,
        req.article.body,
        article_meta.article_id,
        auth_user.user_id
    )
    .fetch_one(&mut tx)
    .await
    .on_constraint("article_slug_key", |_| {
        Error::unprocessable_entity([(
            "slug",
            format!("duplicate article slug: {}", new_slug.unwrap()),
        )])
    })?
    .into_article();

    // Mustn't forget this!
    tx.commit().await?;

    Ok(Json(ArticleBody { article }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#delete-article
async fn delete_article(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Path(slug): Path<String>,
) -> Result<()> {
    let result = sqlx::query!(
        // I like to use raw strings for most queries mainly because CLion doesn't try
        // to escape newlines.
        // language=PostgreSQL
        r#"
            -- The main query cannot observe side-effects of data-modifying CTEs and 
            -- by design, always sees the "before" picture of the database,
            -- so this lets us fold our permissions check together with the actual delete.
            --
            -- This was the "being too clever" I was talking about before. However, I think it's
            -- permissible here as we're not pairing this together with a huge select, so it
            -- should be relatively easy to understand the intended effect here.
            with deleted_article as (
                delete from article 
                -- Important: we only delete the article if the user actually authored it.
                where slug = $1 and user_id = $2
                -- We just need to return *something* for `exists()` below.
                returning 1
            )
            select
                -- This will be `true` if the article existed before we deleted it.
                exists(select 1 from article where slug = $1) "existed!",
                -- This will only be `true` if we actually deleted the article.
                exists(select 1 from deleted_article) "deleted!"
        "#,
        slug,
        auth_user.user_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if result.deleted {
        // Article successfully deleted!
        Ok(())
    } else if result.existed {
        // We found the article, but the user was not the author of that article.
        Err(Error::Forbidden)
    } else {
        // We didn't find any article by the given slug.
        Err(Error::NotFound)
    }
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-article
async fn get_article(
    // The spec states "no authentication required" but should probably state
    // "authentication optional" because we still need to check if the user is following the author.
    maybe_auth_user: MaybeAuthUser,
    ctx: Extension<ApiContext>,
    Path(slug): Path<String>,
) -> Result<Json<ArticleBody>> {
    let article = sqlx::query_as!(
        ArticleFromQuery,
        // language=PostgreSQL
        r#"
            select
                slug,
                title,
                description,
                body,
                tag_list,
                article.created_at "created_at: Timestamptz",
                article.updated_at "updated_at: Timestamptz",
                exists(select 1 from article_favorite where user_id = $1) "favorited!",
                coalesce(
                    -- `count(*)` returns `NULL` if the query returned zero columns
                    -- not exactly a fan of that design choice but whatever
                    (select count(*) from article_favorite fav where fav.article_id = article.article_id),
                    0
                ) "favorites_count!",
                author.username author_username,
                author.bio author_bio,
                author.image author_image,
                exists(select 1 from follow where followed_user_id = author.user_id and following_user_id = $1) "following_author!"
            from article
            inner join "user" author using (user_id)
            where slug = $2
        "#,
        maybe_auth_user.user_id(),
        slug
    )
        .fetch_optional(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?
        .into_article();

    Ok(Json(ArticleBody { article }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#favorite-article
async fn favorite_article(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Path(slug): Path<String>,
) -> Result<Json<ArticleBody>> {
    // This is kind of where the pattern of "always return the updated object" gets a bit annoying,
    // because it makes this handler and `unfavorite_article()` a lot more complicated than they
    // need to be.
    //
    // Fortunately, we can deduplicate the article lookup with a function. We might prefer
    // to do this to `update_article()` as well, but I wanted to demonstrate how you can use
    // a CTE to implement that.

    let article_id = sqlx::query_scalar!(
        r#"
            with selected_article as (
                select article_id from article where slug = $1
            ),
            inserted_favorite as (
                insert into article_favorite(article_id, user_id)
                select article_id, $2
                from selected_article
                -- if the article is already favorited
                on conflict do nothing
            )
            select article_id from selected_article
        "#,
        slug,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or(Error::NotFound)?;

    Ok(Json(ArticleBody {
        article: article_by_id(&ctx.db, auth_user.user_id, article_id).await?,
    }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#unfavorite-article
async fn unfavorite_article(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Path(slug): Path<String>,
) -> Result<Json<ArticleBody>> {
    // The Realworld spec doesn't say what to do if the user calls this on an article
    // that they haven't favorited. I've chosen to just do nothing as that's the easiest.
    //
    // The Postman collection doesn't test that case.

    let article_id = sqlx::query_scalar!(
        r#"
            with selected_article as (
                select article_id from article where slug = $1
            ),
            deleted_favorite as (
                delete from article_favorite
                where article_id = (select article_id from selected_article)
                and user_id = $2
            )
            select article_id from selected_article
        "#,
        slug,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or(Error::NotFound)?;

    Ok(Json(ArticleBody {
        article: article_by_id(&ctx.db, auth_user.user_id, article_id).await?,
    }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#get-tags
async fn get_tags(ctx: Extension<ApiContext>) -> Result<Json<TagsBody>> {
    // Note: this query requires a full table scan and is a likely point for a DoS attack.
    //
    // In practice, I might consider storing unique tags in their own table and then the
    // `tag_list` of an article would be a list of indexes into that table, and then
    // this query can just dump that table. I have not implemented that here for the sake of brevity
    // in the other queries fetching from the `article` table.
    //
    // Alternatively you could store the unique list of tags as a materialized view that is
    // periodically refreshed, or cache the result of this query in application code,
    // or simply apply a global rate-limit to this route. Each has its tradeoffs.
    let tags = sqlx::query_scalar!(
        r#"
            select distinct tag "tag!"
            from article, unnest (article.tag_list) tags(tag)
            order by tag
        "#
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(TagsBody { tags }))
}

// End handler functions.
// Begin utility functions.

// This is used in a few different places so it makes sense to extract into its own function.
//
// I usually throw stuff like this at the bottom of the file but other engineers like
// to put these kinds of functions in their own modules. Po-tay-to po-tah-to.
async fn article_by_id(
    e: impl Executor<'_, Database = Postgres>,
    user_id: Uuid,
    article_id: Uuid,
) -> Result<Article> {
    let article = sqlx::query_as!(
        ArticleFromQuery,
        // language=PostgreSQL
        r#"
            select
                slug,
                title,
                description,
                body,
                tag_list,
                article.created_at "created_at: Timestamptz",
                article.updated_at "updated_at: Timestamptz",
                exists(select 1 from article_favorite where user_id = $1) "favorited!",
                coalesce(
                    -- `count(*)` returns `NULL` if the query returned zero columns
                    -- not exactly a fan of that design choice but whatever
                    (select count(*) from article_favorite fav where fav.article_id = article.article_id),
                    0
                ) "favorites_count!",
                author.username author_username,
                author.bio author_bio,
                author.image author_image,
                exists(select 1 from follow where followed_user_id = author.user_id and following_user_id = $1) "following_author!"
            from article
            inner join "user" author using (user_id)
            where article_id = $2
        "#,
        user_id,
        article_id
    )
        .fetch_optional(e)
        .await?
        .ok_or(Error::NotFound)?
        .into_article();

    Ok(article)
}

/// Convert a title string to a slug for identifying an article.
///
/// E.g. `slugify("Doctests are the Bee's Knees") == "doctests-are-the-bees-knees"`
///
// (Sadly, doctests are not run on private functions it seems.)
fn slugify(string: &str) -> String {
    const QUOTE_CHARS: &[char] = &['\'', '"'];

    string
        // Split on anything that isn't a word character or quotation mark.
        // This has the effect of keeping contractions and possessives together.
        .split(|c: char| !(QUOTE_CHARS.contains(&c) || c.is_alphanumeric()))
        // If multiple non-word characters follow each other then we'll get empty substrings
        // so we'll filter those out.
        .filter(|s| !s.is_empty())
        .map(|s| {
            // Remove quotes from the substring.
            //
            // This allocation is probably avoidable with some more iterator hackery but
            // at that point we'd be micro-optimizing. This function isn't called all that often.
            let mut s = s.replace(QUOTE_CHARS, "");
            // Make the substring lowercase (in-place operation)
            s.make_ascii_lowercase();
            s
        })
        .join("-")
}

// This fulfills the "at least one unit test" requirement of the Realworld spec.
//
// While opinions vary, in general, we're not big fans of TDD at Launchbadge,
// because often you spend most of your time thinking about how you're going to test your code,
// as opposed to getting the job done. When you're on a client's dime, that's really important.
//
// At the same time, you're making your code more difficult to read and reason about because
// you're forced to separate the code from its dependencies for testing.
//
// For example, most of the handler functions in this API touch the database, which isn't
// conducive to unit testing. Sure, you could mock those database calls out but then there's
// really not whole lot left to test. For what little is left, the logic should ideally
// be self-evident, and then testing is just superfluous.
//
// Of course, testing is still really important. Manually testing the API every time you make
// a change only goes so far, can become really unwieldy, and is easy to forget or neglect
// to do because of that.
//
// I'm personally a big proponent of unit-testing only what makes sense to unit-test,
// such as self-contained functions like `slugify()`. The rest can be covered with integration
// or end-to-end testing, which we do a lot of at Launchbadge. That has the advantage of not
// only covering the API, but the frontend as well.
//
// Fortunately, the Realworld spec comes with an API integration test suite already, although
// in many places it doesn't cover much more than just the happy paths. I wish I had the time
// and energy to help fill that out.
#[test]
fn test_slugify() {
    assert_eq!(
        slugify("Segfaults and You: When Raw Pointers Go Wrong"),
        "segfaults-and-you-when-raw-pointers-go-wrong"
    );

    assert_eq!(
        slugify("Why are DB Admins Always Shouting?"),
        "why-are-db-admins-always-shouting"
    );

    assert_eq!(
        slugify("Converting to Rust from C: It's as Easy as 1, 2, 3!"),
        "converting-to-rust-from-c-its-as-easy-as-1-2-3"
    )
}

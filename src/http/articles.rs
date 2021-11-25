use axum::extract::Extension;
use axum::routing::post;
use axum::{Json, Router};

use crate::http::extractor::AuthUser;
use crate::http::profiles::Profile;
use crate::http::types::Timestamptz;
use crate::http::{ApiContext, Error, Result, ResultExt};
use itertools::Itertools;

pub fn router() -> Router {
    Router::new().route("/api/articles", post(create_article))
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ArticleBody<T> {
    article: T,
}

#[derive(serde::Deserialize)]
// The Realworld spec doesn't mention this as an API convention, it just finally shows up
// when you're looking at the spec for the Article object.
#[serde(rename_all = "camelCase")]
struct CreateArticle {
    title: String,
    description: String,
    body: String,
    tag_list: Vec<String>,
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
    updated_at: Option<Timestamptz>,
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
    updated_at: Option<Timestamptz>,
    favorited: bool,
    favorites_count: i64,
    author_username: String,
    author_bio: String,
    author_image: Option<String>,
    author_following: bool,
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
                following: self.author_following,
            },
        }
    }
}

// https://gothinkster.github.io/realworld/docs/specs/backend-specs/endpoints#create-article
async fn create_article(
    auth_user: AuthUser,
    ctx: Extension<ApiContext>,
    Json(req): Json<ArticleBody<CreateArticle>>,
) -> Result<Json<ArticleBody<Article>>> {
    let slug = slugify(&req.article.title);

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
                false "author_following!"
            from inserted_article
            inner join "user" on user_id = $1
        "#,
        auth_user.user_id,
        slug,
        req.article.title,
        req.article.description,
        req.article.body,
        // The typechecking code that SQLx emits for parameters sometimes chokes on vectors.
        // This slicing operation shouldn't be required, but it took a mess of typesystem
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

/// Convert a title string to a slug for identifying an article.
///
/// E.g. `slugify("Doctests are the Bee's Knees") == "doctests-are-the-bees-knees"`
fn slugify(string: &str) -> String {
    const QUOTE_CHARS: &[char] = &['\'', '"'];

    string
        // Split on anything that isn't a word character or quotation mark.
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
// In general, we're not big fans of TDD at Launchbadge, because often you spend most of your time
// thinking about how you're going to test your code, as opposed to getting the job done. At the
// same time, you're making your code more difficult to read and reason about because
// you're forced to separate the code from its dependencies for testing.
//
// For example, most of the handler functions in this API touch the database, which isn't
// conducive to unit testing. Sure, you could mock those database calls out but then there's
// really not whole lot left to test. For what little is left, the logic should ideally
// be self-evident, and then testing is just superfluous.
//
// Instead, we're big proponents of unit-testing what makes sense to unit-test,
// such as self-contained functions like `slugify()`. The rest can be covered with integration
// testing, which fortunately the Realworld spec comes with an API integration test suite already.
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

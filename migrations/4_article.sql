create table article
(
    article_id  uuid primary key     default uuid_generate_v1mc(),

    user_id     uuid        not null references "user" (user_id) on delete cascade,

    -- An article slug appears to be the title stripped down to words separated by hyphens.
    --
    -- As it is used to look up an article, it must be unique.
    --
    -- If we wanted to codify the slugification functionality in Postgres, we could make this a generated column,
    -- but it makes more sense to me to do that in Rust code so it can be unit tested.
    slug        text unique not null,

    title       text        not null,

    description text        not null,
    body        text        not null,

    -- Postgres lets us take the lazy way out and just store the tags as an array, which I'm a big fan of.
    --
    -- We can use a GIN index to speed up searches by tags, though the `GET /api/tags` call will need to do
    -- a full table scan. If I was worried about the performance of that, I would store unique tags in their own table
    -- and then this would be an array of IDs into that table.
    tag_list        text[]      not null,

    -- These fields are actually in the Realworld spec so we will be making use of them.
    created_at  timestamptz not null default now(),
    updated_at  timestamptz
);

select trigger_updated_at('article');

-- This should speed up searching with tags.
create index article_tags_gin on article using gin(tag_list);

-- This table is much more clearly a cousin table of `article` so it's named as such.
create table article_favorite
(
    article_id uuid not null references article(article_id) on delete cascade,
    user_id uuid not null references "user"(user_id) on delete cascade,

    created_at  timestamptz not null default now(),
    updated_at  timestamptz,

    -- Enforce uniqueness like with `follow`.
    primary key (article_id, user_id)

    -- Unlike with follows, it's more than a simple check constraint to forbid an author from favoriting their own
    -- article. Since the Realworld spec doesn't say either way, and it's foreseeable that an author might
    -- want to favorite some of their own articles like "these are my best works", we'll allow it.
);

select trigger_updated_at('article_favorite');
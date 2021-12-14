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
    tag_list    text[]      not null,

    -- These fields are actually in the Realworld spec so we will be making use of them.
    created_at  timestamptz not null default now(),

    -- The Realworld spec requires this to always be set,
    -- but we prefer to leave it null unless the row has actually been updated.
    -- It saves space as well as informs us whether a row has ever been updated or not.
    updated_at timestamptz not null default now()
);

select trigger_updated_at('article');

-- This should speed up searching with tags.
create index article_tags_gin on article using gin (tag_list);

-- This table is much more clearly a cousin table of `article` so it's named as such.
create table article_favorite
(
    article_id uuid        not null references article (article_id) on delete cascade,
    user_id    uuid        not null references "user" (user_id) on delete cascade,

    created_at timestamptz not null default now(),
    updated_at timestamptz,

    -- Enforce uniqueness like with `follow`.
    primary key (article_id, user_id)

    -- Unlike with follows, it's more than a simple check constraint to forbid an author from favoriting their own
    -- article. Since the Realworld spec doesn't say either way, and it's foreseeable that an author might
    -- want to favorite some of their own articles like "these are my best works", we'll allow it.
);

select trigger_updated_at('article_favorite');

-- It's debatable whether this should be `article_comment` or just `comment` as there's no other comment types
-- in the spec. However, this naming choice _allows_ other comment types (e.g. comments directly on a user's profile)
-- to be added later without any refactoring needed. Something to think about.
create table article_comment
(
    -- The Realworld spec shows that it expects comment IDs to be integers, which suggests maybe a `BIGSERIAL`
    -- primary key column. However, the guarantees Postgres makes about sequences are very shaky at best.
    -- https://wiki.postgresql.org/wiki/FAQ#Why_are_there_gaps_in_the_numbering_of_my_sequence.2FSERIAL_column.3F_Why_aren.27t_my_sequence_numbers_reused_on_transaction_abort.3F
    --
    --TL;DR: not guaranteed to be gapless or in a proper order, and attempting to make them have these properties
    -- involves a significant performance hit.
    --
    -- Because the spec expects an integer, we're going to use `bigserial` anyway, but it should _not_ be used
    -- to order the comments. Instead, we'll use `created_at`.
    --
    -- This is one of the handful of places where the Realworld spec is disappointingly naive, or at the very least
    -- rather myopic, as it appears that MySQL's `AUTO_INCREMENT` marker does make these guarantees at the cost
    -- of raw insert performance. I'd hazard a guess that the original implementation used a MySQL database, and the
    -- author just used `AUTO_INCREMENT` PKs for every table, which is a pretty outdated practice these days.
    --
    -- At the end of the day, though, it appears that the spec doesn't really care about these properties either way.
    -- If the spec treated comment IDs as an opaque string then the choice could at least be left up to the
    -- implementation.
    --
    -- In practice, I would just assign comments a UUID like everything else.
    comment_id bigserial primary key,

    article_id uuid        not null references article (article_id) on delete cascade,

    user_id    uuid        not null references "user" (user_id) on delete cascade,

    body       text        not null,

    created_at timestamptz not null default now(),

    -- Same thing here.
    updated_at timestamptz not null default now()
);

select trigger_updated_at('article_comment');

-- This is going to be the primary lookup method so it's not a bad idea to pre-emptively create an index for it,
-- as Postgres wouldn't otherwise do it by default.
create index on article_comment (article_id, created_at);
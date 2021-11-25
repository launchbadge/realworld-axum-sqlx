create table follow
(
    -- By default, "references" (foreign key) relationships will throw errors if the row in the referenced table
    -- is deleted before this one. The `on delete cascade` clause causes this to be automatically deleted if the
    -- corresponding user row is deleted.
    --
    -- Before applying `on delete cascade` to a foreign key clause, though, you should consider the actual semantics
    -- of that table. Does it, for example, contain purchase records that are linked to a payment processor? You may not
    -- want to delete those records for auditing purposes, even if you want to delete the user record itself.
    --
    -- In cases like that, I usually just forego the foreign-key clause and treat the user ID as a plain data column
    -- so the row sticks around even if the user is deleted. There's also `on delete set null` but then that
    -- requires the column to be nullable which makes it unwieldy in queries when it should not be null 99% of the time.
    followed_user_id  uuid        not null references "user" (user_id) on delete cascade,

    -- The naming of this and the previous column was chosen to try to be as unambiguous in queries as possible.
    -- I have also done this before as a `user_follow` table with `user_id` and `following_user_id` columns,
    -- but that had issues with ambiguity in queries that involved looking up who a particular user was following
    -- as the inclination is to join on `user_id` even though that's the wrong column in that case.
    following_user_id uuid        not null references "user" (user_id) on delete cascade,

    created_at        timestamptz not null default now(),

    -- We don't really need an `updated_at` column because there isn't anything to update here.
    -- However, columns with nulls take up very little extra space on-disk in Postgres so it's worth adding
    -- for posterity anyway. In one project that had a "follow this user" feature, there was extra mutable data
    -- on the row in the "follow" table, so there are practical reasons to have this column.
    --
    -- It can also serve as a canary for queries that are modifying this table in weird ways (as normally you'd
    -- expect this to always be null, so seeing this set to a value may be a red flag).
    updated_at timestamptz,

    -- It's debatable whether these kinds of checks belong in application code or in the database.
    -- My policy is to only place a check constraint in the database if it would be unwieldy or
    -- easy to forget to enforce in application code, and the condition isn't expected to change.
    --
    -- Validation of inputs can change with business requirements, but pure sanity checks like this
    -- make sense to keep in the database as I don't foresee a business requirement of
    -- allowing a user to follow themself.
    constraint user_cannot_follow_self check (followed_user_id != following_user_id),

    -- This enforces uniqueness of the (following, followed) pair.
    -- The `following_user_id` is placed first as that allows this index to serve "who am I following" queries,
    -- which exist in the Realworld spec but "who is following me" queries don't.
    primary key (following_user_id, followed_user_id)
);

SELECT trigger_updated_at('follow');
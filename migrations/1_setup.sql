-- This is a boilerplate migration file that we use in nearly every project.
-- It sets up database features that we use quite often.

-- As a style choice, we prefer not to write SQL in all uppercase as lowercase feels friendlier to the eyes.
-- It's nicer to read WHEN THE CODE ISN'T YELLING AT YOU ALL DAY.
-- It perhaps made sense back when code highlighting was not the norm and case was used to differentiate keywords
-- from non-keywords, but at this point it's purely from inertia.
-- The language itself is not case-sensitive except for quoted identifiers.
-- Whichever style you use, however, consistency should still be maintained.

-- This extension gives us `uuid_generate_v1mc()` which generates UUIDs that cluster better than `gen_random_uuid()`
-- while still being difficult to predict and enumerate.
-- Also, while unlikely, `gen_random_uuid()` can in theory produce collisions which can trigger spurious errors on
-- insertion, whereas it's much less likely with `uuid_generate_v1mc()`.
create extension if not exists "uuid-ossp";

-- We try to ensure every table has `created_at` and `updated_at` columns, which can help immensely with debugging
-- and auditing.
--
-- While `created_at` can just be `default now()`, setting `updated_at` on update requires a trigger which
-- is a lot of boilerplate. These two functions save us from writing that every time as instead we can just do
--
-- select trigger_updated_at('<table name>');
--
-- after a `CREATE TABLE`.
create or replace function set_updated_at()
    returns trigger as
$$
begin
    NEW.updated_at = now();
    return NEW;
end;
$$ language plpgsql;

create or replace function trigger_updated_at(tablename regclass)
    returns void as
$$
begin
    execute format('CREATE TRIGGER set_updated_at
        BEFORE UPDATE
        ON %s
        FOR EACH ROW
        WHEN (OLD is distinct from NEW)
    EXECUTE FUNCTION set_updated_at();', tablename);
end;
$$ language plpgsql;

-- Finally, this is a text collation that sorts text case-insensitively, useful for `UNIQUE` indexes
-- over things like usernames and emails, without needing to remember to do case-conversion.
create collation case_insensitive (provider = icu, locale = 'und-u-ks-level2', deterministic = false);

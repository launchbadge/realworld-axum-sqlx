-- This is a boilerplate migration file that we use in nearly every project.
-- It sets up database features that we use quite often.

-- This extension gives us `uuid_generate_v1mc()` which generates UUIDs that cluster better than `gen_random_uuid()`
-- while still being difficult to predict.
-- Also, while unlikely, `gen_random_uuid()` can in theory produce collisions which can trigger spurious errors on
-- insertion, whereas it's much less likely with `uuid_generate_v1mc()`.
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- We try to ensure every table has `created_at` and `updated_at` columns, which can help immensely with debugging
-- and auditing.
--
-- While `created_at` can just be `default now()`, setting `updated_at` on update requires a trigger which
-- is a lot of boilerplate. These two functions save us from writing that every time as instead we can just do
--
-- select trigger_updated_at('<table name>');
--
-- after a `CREATE TABLE`.
CREATE OR REPLACE FUNCTION set_updated_at()
    RETURNS TRIGGER AS
$$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION trigger_updated_at(tablename regclass)
    RETURNS VOID AS
$$
BEGIN
    EXECUTE format('CREATE TRIGGER set_updated_at
        BEFORE UPDATE
        ON %s
        FOR EACH ROW
        WHEN (OLD is distinct from NEW)
    EXECUTE FUNCTION set_updated_at();', tablename);
END;
$$ LANGUAGE plpgsql;

-- Finally, this is a text collation that sorts text case-insensitively, useful for `UNIQUE` indexes
-- over things like usernames and emails, without needing to remember to do case-conversion.
CREATE COLLATION case_insensitive (provider = icu, locale = 'und-u-ks-level2', deterministic = false);

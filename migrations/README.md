### Migrations
SQLx manages and applies changes to the database schema using files called _migrations_.

These are simply SQL script files that are run in a particular order. The filenames follow this pattern:

```
<version number>_<name with words separated by underscores>.sql
```

`.up` or `.down` can also appear before `.sql` which signifies that the migration is _reversible_. However,
you don't need to worry about those for this project.

The version number can be any unique non-decreasing positive integer. `sqlx migrate add` by default uses the current 
UNIX timestamp in seconds as the version number, but you can also use your own scheme. 

What I like to do is to use integers starting from 1 for the "bootstrapping" migrations that initially set up the 
database schema, and then switch to timestamps for new migrations after the first deployment of the project. This
provides a clear delineation between the "original vision" of the project and changes that came after the MVP,
which may be useful for retrospectives. If you have a lot of migrations after the original set, that may indicate
a lot of feature creep on the project or an overly vague initial specification.

If you have multiple developers contributing migrations at the same time, then timestamps may be preferable so that
you don't have to deal with conflicts in version numbers. That is mainly why it's the default for `sqlx migrate add`.

On application startup or when `sqlx migrate run` is executed, SQLx will check with a special `_sqlx_migrations` table
in the database to see which migrations have been applied and which haven't, and apply all that haven't been
in ascending order by their version number.

To ensure the database schema is always reproducible, SQLx also stores the content hash of applied migrations and
checks them against the current contents of the files, so you **must not** change migrations that have already been applied.
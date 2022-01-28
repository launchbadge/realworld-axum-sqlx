use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Formatter;
use time::{Format, OffsetDateTime};

/// `OffsetDateTime` provides RFC-3339 (ISO-8601 subset) serialization, but the default
/// `serde::Serialize` implementation produces array of integers, which is great for binary
/// serialization, but infeasible to consume when returned from an API, and certainly
/// not human-readable.
///
/// With this wrapper type, we override this to provide the serialization format we want.
///
/// `chrono::DateTime` doesn't need this treatment, but Chrono sadly seems to have stagnated,
/// and has a few more papercuts than I'd like:
///
/// * Having to import both `DateTime` and `Utc` everywhere gets annoying quickly.
/// * lack of `const fn` constructors anywhere (especially for `chrono::Duration`)
/// * `cookie::CookieBuilder` (used by Actix-web and `tower-cookies`) bakes-in `time::Duration`
///   for setting the expiration
///     * not really Chrono's fault but certainly doesn't help.
#[derive(sqlx::Type)]
pub struct Timestamptz(pub OffsetDateTime);

impl Serialize for Timestamptz {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0.lazy_format(Format::Rfc3339))
    }
}

impl<'de> Deserialize<'de> for Timestamptz {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrVisitor;

        // By providing our own `Visitor` impl, we can access the string data without copying.
        //
        // We could deserialize a borrowed `&str` directly but certain deserialization modes
        // of `serde_json` don't support that, so we'd be forced to always deserialize `String`.
        //
        // `serde_with` has a helper for this but it can be a bit overkill to bring in
        // just for one type: https://docs.rs/serde_with/latest/serde_with/#displayfromstr
        //
        // We'd still need to implement `Display` and `FromStr`, but those are much simpler
        // to work with.
        //
        // However, I also wanted to demonstrate that it was possible to do this with Serde alone.
        impl Visitor<'_> for StrVisitor {
            type Value = Timestamptz;

            fn expecting(&self, f: &mut Formatter) -> std::fmt::Result {
                f.pad("expected string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                OffsetDateTime::parse(v, Format::Rfc3339)
                    .map(Timestamptz)
                    .map_err(E::custom)
            }
        }

        deserializer.deserialize_str(StrVisitor)
    }
}

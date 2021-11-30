use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Formatter;
use time::{Format, OffsetDateTime};

/// `OffsetDateTime` provides RFC-3339 (ISO-8601 subset) serialization, but the default
/// `serde::Serialize` implementation produces array of integers, which is great for binary
/// serialization but very difficult to consume when returned from an API.
///
/// With this wrapper type, we override this to provide the serialization format we want.
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

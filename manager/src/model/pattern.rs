use aib_core::surt::Surt;
use serde::{
    de::{Deserialize, Deserializer, Unexpected, Visitor},
    ser::{Serialize, SerializeStruct, Serializer},
};
use sqlx::{ColumnIndex, Decode, FromRow, Row, Type};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Stats {
    pub indexed: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Pattern {
    pub id: Option<u64>,
    pub surt: Surt,
    pub name: String,
    pub slug: String,
    pub sort_id: u32,
    pub prefix: bool,
    pub stats: Option<Stats>,
}

impl PartialOrd for Pattern {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pattern {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_id.cmp(&other.sort_id)
    }
}

impl Pattern {
    pub fn url_query(&self) -> String {
        let mut result = self.surt.canonical_url().to_string();

        if self.prefix {
            result.push('*');
        }

        result
    }

    fn field_len(&self) -> usize {
        let mut len = 4;
        if self.id.is_some() {
            len += 1;
        }
        if self.stats.is_some() {
            len += 1;
        }
        len
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const FIELDS: &[&str] = &["id", "query", "name", "slug", "sort", "stats"];
        enum Field {
            Id,
            Query,
            Name,
            Slug,
            Sort,
            Stats,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("Pattern field")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "id" => Ok(Field::Id),
                            "query" => Ok(Field::Query),
                            "name" => Ok(Field::Name),
                            "slug" => Ok(Field::Slug),
                            "sort" => Ok(Field::Sort),
                            "stats" => Ok(Field::Stats),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct PatternVisitor;

        impl<'de> Visitor<'de> for PatternVisitor {
            type Value = Pattern;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Pattern")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut id = None;
                let mut query = None;
                let mut name = None;
                let mut slug = None;
                let mut sort = None;
                let mut stats = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Id => {
                            if id.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id = Some(map.next_value()?);
                        }
                        Field::Query => {
                            if query.is_some() {
                                return Err(serde::de::Error::duplicate_field("query"));
                            }
                            query = Some(map.next_value::<std::borrow::Cow<str>>()?);
                        }
                        Field::Name => {
                            if name.is_some() {
                                return Err(serde::de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value::<std::borrow::Cow<str>>()?);
                        }
                        Field::Slug => {
                            if slug.is_some() {
                                return Err(serde::de::Error::duplicate_field("slug"));
                            }
                            slug = Some(map.next_value::<std::borrow::Cow<str>>()?);
                        }
                        Field::Sort => {
                            if sort.is_some() {
                                return Err(serde::de::Error::duplicate_field("sort"));
                            }
                            sort = Some(map.next_value()?);
                        }
                        Field::Stats => {
                            if stats.is_some() {
                                return Err(serde::de::Error::duplicate_field("stats"));
                            }
                            stats = Some(map.next_value()?);
                        }
                    }
                }

                let query = query.ok_or_else(|| serde::de::Error::missing_field("query"))?;
                let name = name.ok_or_else(|| serde::de::Error::missing_field("name"))?;
                let slug = slug.ok_or_else(|| serde::de::Error::missing_field("slug"))?;
                let sort = sort.ok_or_else(|| serde::de::Error::missing_field("sort"))?;

                let (surt_part, prefix) = if query.ends_with('*') {
                    (query[0..query.len() - 1].into(), true)
                } else {
                    (query, false)
                };

                let surt = Surt::from_url(&surt_part).map_err(|_| {
                    serde::de::Error::invalid_value(Unexpected::Str(&surt_part), &self)
                })?;

                Ok(Pattern {
                    id,
                    surt,
                    prefix,
                    name: name.to_string(),
                    slug: slug.to_string(),
                    sort_id: sort,
                    stats,
                })
            }
        }

        deserializer.deserialize_struct("Pattern", FIELDS, PatternVisitor)
    }
}
impl Serialize for Pattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut result = serializer.serialize_struct("Pattern", self.field_len())?;
        if let Some(id) = &self.id {
            result.serialize_field("id", id)?;
        }
        result.serialize_field("query", &self.url_query())?;
        result.serialize_field("name", &self.name)?;
        result.serialize_field("slug", &self.slug)?;
        result.serialize_field("sort", &self.sort_id)?;
        if let Some(stats) = &self.stats {
            result.serialize_field("stats", stats)?;
        }
        result.end()
    }
}

impl<'r, R: Row> FromRow<'r, R> for Pattern
where
    for<'a> &'a str: ColumnIndex<R>,
    i64: Decode<'r, R::Database>,
    i64: Type<R::Database>,
    bool: Decode<'r, R::Database>,
    bool: Type<R::Database>,
    &'r str: Decode<'r, R::Database>,
    &'r str: Type<R::Database>,
{
    fn from_row(row: &'r R) -> Result<Self, sqlx::Error> {
        let id = row.try_get::<i64, _>("pattern_id")?;
        let surt_str = row.try_get::<&str, _>("surt")?;
        let prefix = row.try_get::<bool, _>("prefix")?;
        let name = row.try_get::<&str, _>("name")?;
        let slug = row.try_get::<&str, _>("slug")?;
        let sort_id = row.try_get::<i64, _>("sort_id")?;
        //let indexed_count = row.try_get::<i64, _>("indexed_count")?;

        Ok(Self {
            id: Some(super::try_cast(id)?),
            surt: surt_str
                .parse()
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
            prefix,
            name: name.to_string(),
            slug: slug.to_string(),
            sort_id: super::try_cast(sort_id)?,
            /*stats: Some(Stats {
                indexed: super::try_cast(indexed_count)?,
            }),*/
            stats: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_serde_json() {
        let pattern = Pattern {
            id: Some(1),
            surt: "com,twitter)/richardbspencer/".parse().unwrap(),
            prefix: true,
            name: "Richard Spencer's Twitter account".to_string(),
            slug: "spencer".to_string(),
            sort_id: 1,
            stats: None,
        };

        let as_json = serde_json::json!(pattern);
        let decoded = serde_json::from_value(as_json).unwrap();

        assert_eq!(pattern, decoded);
    }
}

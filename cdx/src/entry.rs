use crate::mime_type::MimeType;
use aib_core::{digest::Digest, surt::Surt, timestamp::Timestamp};
use serde::de::{Deserialize, Deserializer, SeqAccess, Unexpected, Visitor};
use std::borrow::Cow;

const EXPECTED_ENTRY_LIST_LEN: usize = 10_000;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("JSON decoding error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid MIME type")]
    InvalidMimeType(#[from] crate::mime_type::Error),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Entry {
    pub key: Surt,
    pub timestamp: Timestamp,
    pub original: String,
    pub mime_type: MimeType,
    pub status_code: Option<u16>,
    pub digest: Digest,
    pub length: u64,
    pub extra_info: Option<ExtraInfo>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExtraInfo {
    pub redirect: String,
    pub robot_flags: String,
    pub offset: u64,
    pub file_name: String,
}

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EntryVisitor;

        impl<'de> Visitor<'de> for EntryVisitor {
            type Value = Entry;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Entry")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Entry, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let key = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let timestamp = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                let original = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                let mime_type = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(3, &self))?;
                let status_code_str: Cow<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(4, &self))?;

                let status_code = if status_code_str == "-" {
                    None
                } else {
                    Some(status_code_str.parse::<u16>().map_err(|_| {
                        serde::de::Error::invalid_value(Unexpected::Str(&status_code_str), &self)
                    })?)
                };

                let digest = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(5, &self))?;

                let length_str_or_redirect: Cow<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(6, &self))?;

                let end_or_robotflags: Option<Cow<str>> = seq.next_element()?;

                match end_or_robotflags {
                    None => {
                        let length = length_str_or_redirect.parse::<u64>().map_err(|_| {
                            serde::de::Error::invalid_value(
                                Unexpected::Str(&length_str_or_redirect),
                                &self,
                            )
                        })?;

                        Ok(Entry {
                            key,
                            timestamp,
                            original,
                            mime_type,
                            status_code,
                            digest,
                            length,
                            extra_info: None,
                        })
                    }
                    Some(robot_flags) => {
                        let length_str: &str = seq
                            .next_element()?
                            .ok_or_else(|| serde::de::Error::invalid_length(8, &self))?;

                        let length = length_str.parse::<u64>().map_err(|_| {
                            serde::de::Error::invalid_value(Unexpected::Str(length_str), &self)
                        })?;

                        let offset_str: &str = seq
                            .next_element()?
                            .ok_or_else(|| serde::de::Error::invalid_length(9, &self))?;

                        let offset = offset_str.parse::<u64>().map_err(|_| {
                            serde::de::Error::invalid_value(Unexpected::Str(offset_str), &self)
                        })?;

                        let file_name = seq
                            .next_element()?
                            .ok_or_else(|| serde::de::Error::invalid_length(10, &self))?;

                        Ok(Entry {
                            key,
                            timestamp,
                            original,
                            mime_type,
                            status_code,
                            digest,
                            length,
                            extra_info: Some(ExtraInfo {
                                redirect: length_str_or_redirect.to_string(),
                                robot_flags: robot_flags.to_string(),
                                offset,
                                file_name,
                            }),
                        })
                    }
                }
            }
        }

        deserializer.deserialize_seq(EntryVisitor)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryList {
    pub values: Vec<Entry>,
}

impl<'de> Deserialize<'de> for EntryList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EntryListVisitor;

        impl<'de> Visitor<'de> for EntryListVisitor {
            type Value = EntryList;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct EntryList")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<EntryList, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let _header: EntryHeader = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let mut entries = Vec::with_capacity(EXPECTED_ENTRY_LIST_LEN);

                while let Some(next) = seq.next_element::<Entry>()? {
                    entries.push(next);
                }

                Ok(EntryList { values: entries })
            }
        }

        deserializer.deserialize_seq(EntryListVisitor)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryHeader {
    Short,
    Full,
}

impl<'de> Deserialize<'de> for EntryHeader {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EntryHeaderVisitor;

        fn validate<'de, V>(value: &str, expected: &'static str) -> Result<(), V::Error>
        where
            V: SeqAccess<'de>,
        {
            if value == expected {
                Ok(())
            } else {
                Err(serde::de::Error::invalid_value(
                    Unexpected::Str(value),
                    &expected,
                ))
            }
        }

        impl<'de> Visitor<'de> for EntryHeaderVisitor {
            type Value = EntryHeader;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct EntryHeader")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<EntryHeader, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let key: Cow<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                validate::<V>(&key, "urlkey")?;

                let timestamp: Cow<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                validate::<V>(&timestamp, "timestamp")?;

                let original: Cow<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                validate::<V>(&original, "original")?;

                let mime_type: Cow<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(3, &self))?;
                validate::<V>(&mime_type, "mimetype")?;

                let status_code: Cow<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(4, &self))?;
                validate::<V>(&status_code, "statuscode")?;

                let digest: Cow<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(5, &self))?;
                validate::<V>(&digest, "digest")?;

                let length_or_redirect: Cow<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(6, &self))?;

                match length_or_redirect.as_ref() {
                    "length" => match seq.next_element::<()>()? {
                        None => Ok(EntryHeader::Short),
                        Some(_) => Err(serde::de::Error::invalid_length(7, &self)),
                    },
                    "redirect" => {
                        let robot_flags: Cow<str> = seq
                            .next_element()?
                            .ok_or_else(|| serde::de::Error::invalid_length(7, &self))?;
                        validate::<V>(&robot_flags, "robotflags")?;

                        let length: Cow<str> = seq
                            .next_element()?
                            .ok_or_else(|| serde::de::Error::invalid_length(8, &self))?;
                        validate::<V>(&length, "length")?;

                        let offset: Cow<str> = seq
                            .next_element()?
                            .ok_or_else(|| serde::de::Error::invalid_length(9, &self))?;
                        validate::<V>(&offset, "offset")?;

                        let file_name: Cow<str> = seq
                            .next_element()?
                            .ok_or_else(|| serde::de::Error::invalid_length(10, &self))?;
                        validate::<V>(&file_name, "filename")?;

                        match seq.next_element::<()>()? {
                            None => Ok(EntryHeader::Full),
                            Some(_) => Err(serde::de::Error::invalid_length(11, &self)),
                        }
                    }
                    other => Err(serde::de::Error::invalid_value(
                        Unexpected::Str(other),
                        &"length",
                    )),
                }
            }
        }

        deserializer.deserialize_seq(EntryHeaderVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_short() {
        let contents = include_str!("../examples/1706619334645856.json");
        let entries = serde_json::from_str::<EntryList>(contents).unwrap();

        assert_eq!(entries.values.len(), 37647);
    }

    #[test]
    fn deserialize_full() {
        let contents = include_str!("../examples/1702374488385081.json");
        let entries = serde_json::from_str::<EntryList>(contents).unwrap();

        assert_eq!(entries.values.len(), 8838);
    }
}

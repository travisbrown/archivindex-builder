use serde::{
    de::{Deserialize, Deserializer, Unexpected, Visitor},
    ser::{Serialize, Serializer},
};
use std::fmt::Display;
use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid SURT")]
    InvalidSurt(String),
    #[error("Invalid domain part")]
    InvalidDomainPart(String),
    #[error("Invalid URL")]
    InvalidUrl(#[from] url::ParseError),
    #[error("Unexpected URL")]
    UnexpectedUrl(String),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Surt {
    pub domain: Vec<String>,
    pub path: String,
}

impl Surt {
    pub fn from_url(input: &str) -> Result<Self, Error> {
        let url: url::Url = input.to_lowercase().parse()?;

        match (url.scheme(), url.domain()) {
            ("http" | "https", Some(domain)) if url.port().is_none() => {
                let mut domain_parts = domain
                    .split('.')
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>();
                domain_parts.reverse();

                Ok(Self {
                    domain: domain_parts,
                    path: url.path().to_string(),
                })
            }
            _ => Err(Error::UnexpectedUrl(input.to_string())),
        }
    }

    pub fn canonical_url(&self) -> SurtCanonicalUrl {
        SurtCanonicalUrl { source: self }
    }
}

impl Display for Surt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.domain.join(","))?;
        f.write_str(")")?;
        f.write_str(&self.path)?;

        Ok(())
    }
}

impl FromStr for Surt {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.to_lowercase() != s {
            Err(Error::InvalidSurt(s.to_string()))
        } else {
            let mut parts = s.split(')');

            let domain = parts
                .next()
                .ok_or_else(|| Error::InvalidSurt(s.to_string()))?;
            let domain_parts = domain.split(',').map(String::from).collect::<Vec<_>>();

            for domain_part in &domain_parts {
                if domain_part
                    .chars()
                    .any(|ch| !ch.is_ascii_alphanumeric() && ch != '-')
                {
                    return Err(Error::InvalidDomainPart(domain_part.to_string()));
                }
            }

            let path = s[domain.len() + 1..].to_string();
            if path.chars().nth(0) != Some('/') {
                return Err(Error::InvalidSurt(s.to_string()));
            }

            Ok(Self {
                domain: domain_parts,
                path,
            })
        }
    }
}

impl<'de> Deserialize<'de> for Surt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SurtVisitor;

        impl<'de> Visitor<'de> for SurtVisitor {
            type Value = Surt;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Surt")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                v.parse()
                    .map_err(|_| serde::de::Error::invalid_value(Unexpected::Str(v), &self))
            }
        }

        deserializer.deserialize_str(SurtVisitor)
    }
}

impl Serialize for Surt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        //serializer.serialize_str(&SurtCanonicalUrl { source: self }.to_string())
        serializer.serialize_str(&self.to_string())
    }
}

pub struct SurtCanonicalUrl<'a> {
    source: &'a Surt,
}

impl<'a> Display for SurtCanonicalUrl<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("https://")?;

        let mut parts = self.source.domain.iter();
        let first = parts.next();

        for domain_part in parts.rev() {
            f.write_str(domain_part)?;
            f.write_str(".")?;
        }

        if let Some(first_part) = first {
            f.write_str(first_part)?;
        }

        f.write_str(&self.source.path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let input = "com,twitter)/farleftwatch/status/999825423977639936";
        let parsed = input.parse::<Surt>().unwrap();

        assert_eq!(parsed.domain.len(), 2);

        let printed = parsed.to_string();

        assert_eq!(input, printed);
    }

    #[test]
    fn from_url() {
        let input = "https://twitter.com/RichardBSpencer/";
        let surt = Surt::from_url(input).unwrap();
        let expected = "com,twitter)/richardbspencer/".parse().unwrap();

        assert_eq!(surt, expected);
    }

    #[test]
    fn canonical_url() {
        let input = "com,twitter)/farleftwatch/status/999825423977639936";
        let parsed = input.parse::<Surt>().unwrap();
        let expected = "https://twitter.com/farleftwatch/status/999825423977639936";

        assert_eq!(parsed.canonical_url().to_string(), expected);
    }
}

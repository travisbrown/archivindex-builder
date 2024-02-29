use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Config {
    items: ItemsConfig,
    cdx: CdxConfig,
    redirects: RedirectsConfig,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ItemsConfig {
    paths: Vec<ItemPath>,
    compression: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct CdxConfig {
    path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RedirectsConfig {
    path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ItemPath {
    prefix: (String, String),
    path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parse_toml() {
        let example = r#"
[items]
paths = [
    { prefix = ["0", "H"], path = "/mnt/data1/items/" },
    { prefix = ["I", "Z"], path = "/mnt/data2/items/" }
]
compression = 14

[cdx]
path = "/mnt/data3/cdx/"

[redirects]
path = "/mnt/data3/redirects/"
        "#;

        let expected = Config {
            items: ItemsConfig {
                paths: vec![
                    ItemPath {
                        prefix: ("0".to_string(), "H".to_string()),
                        path: Path::new("/mnt/data1/items/").to_path_buf(),
                    },
                    ItemPath {
                        prefix: ("I".to_string(), "Z".to_string()),
                        path: Path::new("/mnt/data2/items/").to_path_buf(),
                    },
                ],
                compression: 14,
            },
            cdx: CdxConfig {
                path: Path::new("/mnt/data3/cdx/").to_path_buf(),
            },
            redirects: RedirectsConfig {
                path: Path::new("/mnt/data3/redirects/").to_path_buf(),
            },
        };

        let parsed: Config = toml::from_str(example).unwrap();

        assert_eq!(parsed, expected);
    }
}

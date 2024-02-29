pub mod config;
pub mod items;
pub mod legacy;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Item store error")]
    Items(#[from] items::Error),
}

#[cfg(test)]
mod tests {
    /*use super::Store;
    use futures::stream::TryStreamExt;

    fn digests() -> Vec<String> {
        vec![
            "2G3EOT7X6IEQZXKSM3OJJDW6RBCHB7YE".to_string(),
            "5DECQVIU7Y3F276SIBAKKCRGDMVXJYFV".to_string(),
            "AJBB526CEZFOBT3FCQYLRMXQ2MSFHE3O".to_string(),
            "Y2A3M6COP2G6SKSM4BOHC2MHYS3UW22V".to_string(),
            "YJFNIRKJZTUBLTRDVCZC5EMUWOOYJN7L".to_string(),
        ]
    }

    fn correct_digest(input: &str) -> String {
        if input == "5DECQVIU7Y3F276SIBAKKCRGDMVXJYFV" {
            "5BPR3OBK6O7KJ6PKFNJRNUICXWNZ46QG".to_string()
        } else {
            input.to_string()
        }
    }

    #[tokio::test]
    async fn compute_digests() {
        let store = Store::new("examples/wayback/store/items/");

        let mut result = store
            .compute_digests(None, 2)
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        result.sort();

        assert_eq!(
            result,
            digests()
                .into_iter()
                .map(|digest| (digest.clone(), correct_digest(&digest)))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn paths() {
        let store = Store::new("examples/wayback/store/items/");

        let mut result = store
            .paths()
            .map(|res| res.map(|p| p.0))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        result.sort();

        assert_eq!(result, digests());
    }

    #[test]
    fn path_for_prefix_1() {
        let store = Store::new("examples/wayback/store/items/");

        let mut result = store
            .paths_for_prefix("Y")
            .map(|res| res.map(|p| p.0))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        result.sort();

        assert_eq!(
            result,
            digests()
                .into_iter()
                .filter(|digest| digest.starts_with("Y"))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn path_for_prefix_2() {
        let store = Store::new("examples/wayback/store/items/");

        let mut result = store
            .paths_for_prefix("YJ")
            .map(|res| res.map(|p| p.0))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        result.sort();

        assert_eq!(
            result,
            digests()
                .into_iter()
                .filter(|digest| digest.starts_with("YJ"))
                .collect::<Vec<_>>()
        );
    }*/
}

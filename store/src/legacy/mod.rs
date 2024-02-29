use flate2::read::GzDecoder;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub mod wayback;

pub fn import_gz<P: AsRef<Path>>(
    path: P,
) -> Result<
    Box<dyn Iterator<Item = Result<(String, BufReader<GzDecoder<File>>), super::items::Error>>>,
    super::items::Error,
> {
    let mut files = std::fs::read_dir(path)?
        .map(|entry| {
            let entry = entry?;
            let file_name = entry
                .file_name()
                .into_string()
                .ok()
                .filter(|file_name| file_name.ends_with(".gz"))
                .ok_or_else(|| super::items::Error::Unexpected(entry.path()))?;

            Ok((file_name.chars().take(32).collect(), entry.path()))
        })
        .collect::<Result<Vec<(String, PathBuf)>, super::items::Error>>()?;

    files.sort();

    Ok(Box::new(files.into_iter().map(|(file_stem, path)| {
        let file = File::open(path)?;
        let reader = BufReader::new(GzDecoder::new(file));

        Ok((file_stem, reader))
    })))
}

use super::item::{columns, Item, ItemWriter};
use parquet::{basic::ZstdLevel, file::properties::WriterProperties};
use parquetry::{write::SchemaWrite, Schema};
use std::path::Path;

const DEFAULT_MAX_CONTENT_BYTES: usize = 512 * 1024 * 1024;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Parquetry error")]
    Parquetry(#[from] parquetry::error::Error),
    #[error("Parquetry value error")]
    ParquetryValue(#[from] parquetry::error::ValueError),
    #[error("I/O error")]
    Io(#[from] std::io::Error),
}

pub struct Writer<W: std::io::Write> {
    writer: ItemWriter<W>,
    max_content_bytes: usize,
}

impl<W: std::io::Write + Send> Writer<W> {
    pub fn write<E: From<Error>, I: Iterator<Item = Result<Item, E>>>(
        &mut self,
        items: I,
    ) -> Result<parquet::format::FileMetaData, E> {
        //self.writer.
        //for item in items {}
        todo![]
    }

    pub fn finish(self) -> Result<parquet::format::FileMetaData, Error> {
        Ok(self.writer.finish()?)
    }
}

impl Writer<std::fs::File> {
    pub fn new<P: AsRef<Path>>(path: P, level: Option<u32>) -> Result<Self, Error> {
        let level = level
            .map_or(Ok(None), |level| ZstdLevel::try_new(level as i32).map(Some))
            .map_err(parquetry::error::Error::from)?;

        Ok(Self {
            writer: Item::writer(
                std::fs::File::create(path)?,
                Self::default_properties(level),
            )?,
            max_content_bytes: DEFAULT_MAX_CONTENT_BYTES,
        })
    }
}

impl<W: std::io::Write> Writer<W> {
    fn default_properties(level: Option<ZstdLevel>) -> WriterProperties {
        let builder = WriterProperties::builder()
            .set_writer_version(parquet::file::properties::WriterVersion::PARQUET_2_0)
            .set_sorting_columns(Some(vec![columns::DIGEST.sorting()]))
            .set_column_dictionary_enabled(columns::DIGEST.path(), false)
            .set_column_dictionary_enabled(columns::CONTENT.path(), false)
            .set_column_encoding(
                columns::DIGEST.path(),
                parquet::basic::Encoding::DELTA_BYTE_ARRAY,
            )
            .set_column_bloom_filter_enabled(columns::DIGEST.path(), true)
            .set_column_encoding(
                columns::CONTENT.path(),
                parquet::basic::Encoding::DELTA_LENGTH_BYTE_ARRAY,
            )
            .set_column_statistics_enabled(
                columns::CONTENT.path(),
                parquet::file::properties::EnabledStatistics::None,
            );

        let builder = match level {
            Some(level) => builder.set_column_compression(
                columns::CONTENT.path(),
                parquet::basic::Compression::ZSTD(level),
            ),
            None => builder,
        };

        builder.build()
    }
}

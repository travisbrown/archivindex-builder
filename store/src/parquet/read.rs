use aib_core::digest::{Sha1Computer, Sha1Digest};
use parquet::{
    column::reader::ColumnReader,
    data_type::{ByteArray, FixedLenByteArray},
    file::{
        properties::ReaderProperties,
        reader::{ChunkReader, FileReader},
        serialized_reader::{ReadOptionsBuilder, SerializedFileReader},
    },
};
use std::collections::HashSet;

const DEFAULT_INITIAL_BUFFER_SIZE: usize = 2048;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Parquet error")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("SHA-1 digest error")]
    Sha1Digest(#[from] aib_core::digest::Error),
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("Invalid digest error")]
    InvalidDigest {
        expected: Sha1Digest,
        found: Sha1Digest,
    },
    #[error("Unexpected digest column reader type error")]
    UnexpectedDigestColumnReaderType,
    #[error("Unexpected content column reader type error")]
    UnexpectedContentColumnReaderType,
    #[error("Missing Bloom filter error")]
    MissingBloomFilter,
    #[error("Mismatched column lengths error")]
    MismatchedColumnLengths {
        digest_records_read: usize,
        content_records_read: usize,
    },
}

pub struct ParquetReader<R: ChunkReader> {
    reader: SerializedFileReader<R>,
    targets: Option<HashSet<Sha1Digest>>,
    digest_computer: Option<Sha1Computer>,
    digest_values: Vec<FixedLenByteArray>,
    content_values: Vec<ByteArray>,
    current_row_group_index: usize,
    current_row_index: Option<usize>,
}

impl<R: ChunkReader + 'static> ParquetReader<R> {
    pub fn new(
        reader: R,
        targets: Option<HashSet<Sha1Digest>>,
        validate_digests: bool,
    ) -> Result<Self, Error> {
        let mut properties = ReaderProperties::builder();

        if targets.is_some() {
            properties = properties.set_read_bloom_filter(true);
        }

        let options = ReadOptionsBuilder::new()
            .with_reader_properties(properties.build())
            .build();

        let digest_computer = if validate_digests {
            Some(Sha1Computer::default())
        } else {
            None
        };

        Ok(Self {
            reader: SerializedFileReader::new_with_options(reader, options)?,
            targets,
            digest_computer,
            digest_values: Vec::with_capacity(DEFAULT_INITIAL_BUFFER_SIZE),
            content_values: Vec::with_capacity(DEFAULT_INITIAL_BUFFER_SIZE),
            current_row_group_index: 0,
            current_row_index: None,
        })
    }

    fn load_row_group(&mut self) -> Result<bool, Error> {
        let row_group_reader = self.reader.get_row_group(self.current_row_group_index)?;

        let has_candidates = self.targets.as_ref().map_or(true, |targets| {
            match row_group_reader.get_column_bloom_filter(0) {
                Some(bloom_filter) => targets
                    .iter()
                    .any(|target| bloom_filter.check(&Sha1DigestWrapper(*target))),
                None => {
                    // We expected a Bloom filter but didn't find one. We could error here but instead we just check the row group.
                    true
                }
            }
        });

        if has_candidates {
            let mut digest_reader = row_group_reader
                .get_column_reader(0)
                .map_err(Error::from)
                .and_then(|reader| match reader {
                    ColumnReader::FixedLenByteArrayColumnReader(reader) => Ok(reader),
                    _other => Err(Error::UnexpectedDigestColumnReaderType),
                })?;

            let mut content_reader = row_group_reader
                .get_column_reader(1)
                .map_err(Error::from)
                .and_then(|reader| match reader {
                    ColumnReader::ByteArrayColumnReader(reader) => Ok(reader),
                    _other => Err(Error::UnexpectedContentColumnReaderType),
                })?;

            self.digest_values.clear();
            self.content_values.clear();

            let (digest_records_read, _digest_values_read, _digest_levels_read) =
                digest_reader.read_records(usize::MAX, None, None, &mut self.digest_values)?;

            let (content_records_read, _content_values_read, _content_levels_read) =
                content_reader.read_records(usize::MAX, None, None, &mut self.content_values)?;

            if digest_records_read != content_records_read {
                return Err(Error::MismatchedColumnLengths {
                    digest_records_read,
                    content_records_read,
                });
            }
        }

        Ok(has_candidates)
    }

    fn check_digest(
        &self,
        digest: Sha1Digest,
        bytes: &ByteArray,
    ) -> Result<(Sha1Digest, Vec<u8>), Error> {
        let bytes = bytes.data().to_vec();

        match self.digest_computer.as_ref() {
            Some(digest_computer) => {
                let mut cursor = std::io::Cursor::new(&bytes);
                let bytes_digest = digest_computer.digest(&mut cursor)?;

                if bytes_digest == digest {
                    Ok((digest, bytes))
                } else {
                    Err(Error::InvalidDigest {
                        expected: digest,
                        found: bytes_digest,
                    })
                }
            }
            None => Ok((digest, bytes)),
        }
    }
}

impl<R: ChunkReader + 'static> Iterator for ParquetReader<R> {
    type Item = Result<(Sha1Digest, Vec<u8>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current_row_index {
            None => {
                while self.current_row_group_index < self.reader.num_row_groups() {
                    match self.load_row_group() {
                        Ok(true) => {
                            // We're ready to start the next row group.
                            self.current_row_index = Some(0);
                            return self.next();
                        }
                        Ok(false) => {
                            // The row group didn't contain any target rows.
                            self.current_row_group_index += 1;
                        }
                        Err(error) => {
                            // There was an error reading the row group.
                            return Some(Err(error));
                        }
                    }
                }

                // We're done.
                None
            }
            Some(mut index) => {
                while index < self.digest_values.len() {
                    match aib_core::digest::Sha1Digest::try_from(self.digest_values[index].data()) {
                        Ok(digest) => {
                            if self
                                .targets
                                .as_ref()
                                .map_or(true, |targets| targets.contains(&digest))
                            {
                                let result = self.check_digest(digest, &self.content_values[index]);

                                self.current_row_index = Some(index + 1);

                                return Some(result);
                            } else {
                                index += 1;
                            }
                        }
                        Err(error) => {
                            return Some(Err(Error::from(error)));
                        }
                    }
                }

                // We're at the end of the row group, so we reset the state and start over.
                self.current_row_group_index += 1;
                self.current_row_index = None;
                self.next()
            }
        }
    }
}

pub fn read_parquet<R: ChunkReader + 'static>(
    file: R,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut total_read_count = 0;

    let properties = ReaderProperties::builder()
        .set_read_bloom_filter(true)
        .build();

    let options = ReadOptionsBuilder::new()
        .with_reader_properties(properties)
        .build();

    let reader = SerializedFileReader::new_with_options(file, options)?;

    let metadata = reader.metadata();
    let mut digest_values = vec![];
    let mut content_values = vec![];
    let digest_computer = aib_core::digest::Sha1Computer::default();

    for index in 0..reader.num_row_groups() {
        let row_group_reader = reader.get_row_group(index)?;
        let mut digest_reader = match row_group_reader.get_column_reader(0)? {
            ColumnReader::FixedLenByteArrayColumnReader(reader) => reader,
            _ => panic!("Bad type"),
        };

        let digest_filter = row_group_reader.get_column_bloom_filter(0).unwrap();

        let target: aib_core::digest::Sha1Digest = "ITQPQIXRSOTGSBHCX5LKZ26YENLPPXYT".parse()?;

        if digest_filter.check(&Sha1DigestWrapper(target)) {
            let mut content_reader = match row_group_reader.get_column_reader(1)? {
                ColumnReader::ByteArrayColumnReader(reader) => reader,
                _ => panic!("Bad type"),
            };

            digest_values.clear();
            content_values.clear();

            let (records_read, values_read, levels_read) =
                digest_reader.read_records(usize::MAX, None, None, &mut digest_values)?;

            cli_helpers::prelude::log::info!(
                "digest records: {}, values: {}, levels: {}",
                records_read,
                values_read,
                levels_read
            );

            let (records_read, values_read, levels_read) =
                content_reader.read_records(usize::MAX, None, None, &mut content_values)?;

            cli_helpers::prelude::log::info!(
                "content records: {}, values: {}, levels: {}",
                records_read,
                values_read,
                levels_read
            );

            cli_helpers::prelude::log::info!(
                "digest: {}, content: {}",
                digest_values.len(),
                content_values.len()
            );

            if digest_values.len() != content_values.len() {
                panic!("lengths don't match");
            } else {
                for (digest, content) in digest_values.iter().zip(&content_values) {
                    let digest = aib_core::digest::Sha1Digest::try_from(digest.data())?;
                    let mut content_cursor = std::io::Cursor::new(content.data());
                    let content_digest = digest_computer.digest(&mut content_cursor)?;

                    if digest != content_digest {
                        panic!("MISMATCH: {}, {}", digest, content_digest);
                    } else {
                        println!("{}", digest);
                    }
                }
            }
        } else {
            cli_helpers::prelude::log::info!("Skipped");
        }

        total_read_count += digest_values.len();
    }

    Ok(total_read_count)
}

fn find_in_parquet<R: ChunkReader + 'static>(
    file: R,
    digest: Sha1Digest,
) -> Result<usize, Box<dyn std::error::Error>> {
    todo![]
}

/// This is a hack to work around the lack of array instances for `parquet::data_type::AsBytes`.
struct Sha1DigestWrapper(Sha1Digest);

impl parquet::data_type::AsBytes for Sha1DigestWrapper {
    fn as_bytes(&self) -> &[u8] {
        self.0 .0.as_slice()
    }
}

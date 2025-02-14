#![cfg_attr(rustfmt, rustfmt_skip)]
const SCHEMA_SOURCE: &str = "message item {
  required fixed_len_byte_array(20) digest;
  required byte_array content;
}";
pub static SCHEMA: std::sync::LazyLock<parquet::schema::types::SchemaDescPtr> = std::sync::LazyLock::new(||
std::sync::Arc::new(
    parquet::schema::types::SchemaDescriptor::new(
        std::sync::Arc::new(
            parquet::schema::parser::parse_message_type(SCHEMA_SOURCE).unwrap(),
        ),
    ),
));
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Item {
    pub digest: [u8; 20],
    pub content: Vec<u8>,
}
pub mod columns {
    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum SortColumn {
        Digest,
        Content,
    }
    impl parquetry::sort::SortColumn for SortColumn {
        fn index(&self) -> usize {
            match self {
                Self::Digest => 0,
                Self::Content => 1,
            }
        }
    }
    pub const DIGEST: parquetry::ColumnInfo = parquetry::ColumnInfo {
        index: 0,
        path: &["digest"],
    };
    pub const CONTENT: parquetry::ColumnInfo = parquetry::ColumnInfo {
        index: 1,
        path: &["content"],
    };
}
impl parquetry::Schema for Item {
    type SortColumn = columns::SortColumn;
    type Writer<W: std::io::Write + Send> = ItemWriter<W>;
    fn sort_key_value(
        &self,
        sort_key: parquetry::sort::SortKey<Self::SortColumn>,
    ) -> Vec<u8> {
        {
            let mut bytes = vec![];
            for column in sort_key.columns() {
                self.write_sort_key_bytes(column, &mut bytes);
            }
            bytes
        }
    }
    fn source() -> &'static str {
        SCHEMA_SOURCE
    }
    fn schema() -> parquet::schema::types::SchemaDescPtr {
        SCHEMA.clone()
    }
    fn writer<W: std::io::Write + Send>(
        writer: W,
        properties: parquet::file::properties::WriterProperties,
    ) -> Result<Self::Writer<W>, parquetry::error::Error> {
        {
            Ok(Self::Writer {
                writer: parquet::file::writer::SerializedFileWriter::new(
                    writer,
                    SCHEMA.root_schema_ptr(),
                    std::sync::Arc::new(properties),
                )?,
                workspace: Default::default(),
            })
        }
    }
}
pub struct ItemWriter<W: std::io::Write> {
    writer: parquet::file::writer::SerializedFileWriter<W>,
    workspace: ParquetryWorkspace,
}
impl<W: std::io::Write + Send> parquetry::write::SchemaWrite<Item, W> for ItemWriter<W> {
    fn write_row_group<
        'a,
        E: From<parquetry::error::Error>,
        I: Iterator<Item = Result<&'a Item, E>>,
    >(
        &mut self,
        values: &mut I,
    ) -> Result<parquet::file::metadata::RowGroupMetaDataPtr, E>
    where
        Item: 'a,
    {
        {
            Item::fill_workspace(&mut self.workspace, values)?;
            Item::write_with_workspace(&mut self.writer, &mut self.workspace)
                .map_err(E::from)
        }
    }
    fn write_item(&mut self, value: &Item) -> Result<(), parquetry::error::Error> {
        Item::add_item_to_workspace(&mut self.workspace, value)
    }
    fn finish_row_group(
        &mut self,
    ) -> Result<parquet::file::metadata::RowGroupMetaDataPtr, parquetry::error::Error> {
        Item::write_with_workspace(&mut self.writer, &mut self.workspace)
    }
    fn finish(self) -> Result<parquet::format::FileMetaData, parquetry::error::Error> {
        Ok(self.writer.close()?)
    }
}
impl TryFrom<parquet::record::Row> for Item {
    type Error = parquetry::error::Error;
    fn try_from(row: parquet::record::Row) -> Result<Self, parquetry::error::Error> {
        {
            let mut fields = row.get_column_iter();
            let digest = match fields
                .next()
                .ok_or_else(|| parquetry::error::Error::InvalidField(
                    "digest".to_string(),
                ))?
                .1
            {
                parquet::record::Field::Bytes(value) => {
                    Ok(
                        value
                            .data()
                            .try_into()
                            .map_err(|_| parquetry::error::Error::InvalidField(
                                "value".to_string(),
                            ))?,
                    )
                }
                _ => Err(parquetry::error::Error::InvalidField("digest".to_string())),
            }?;
            let content = match fields
                .next()
                .ok_or_else(|| parquetry::error::Error::InvalidField(
                    "content".to_string(),
                ))?
                .1
            {
                parquet::record::Field::Bytes(value) => Ok(value.data().to_vec()),
                _ => Err(parquetry::error::Error::InvalidField("content".to_string())),
            }?;
            Ok(Item { digest, content })
        }
    }
}
impl Item {
    fn write_sort_key_bytes(
        &self,
        column: parquetry::sort::Sort<<Self as parquetry::Schema>::SortColumn>,
        bytes: &mut Vec<u8>,
    ) {
        match column.column {
            columns::SortColumn::Digest => {
                let value = self.digest;
                for b in value {
                    bytes.push(if column.descending { !b } else { b });
                }
            }
            columns::SortColumn::Content => {
                let value = &self.content;
                for b in value {
                    bytes.push(if column.descending { !b } else { *b });
                }
            }
        }
    }
    fn write_with_workspace<W: std::io::Write + Send>(
        file_writer: &mut parquet::file::writer::SerializedFileWriter<W>,
        workspace: &mut ParquetryWorkspace,
    ) -> Result<parquet::file::metadata::RowGroupMetaDataPtr, parquetry::error::Error> {
        {
            let mut row_group_writer = file_writer.next_row_group()?;
            let mut column_writer = row_group_writer
                .next_column()?
                .ok_or_else(|| parquetry::error::Error::InvalidField(
                    "digest".to_string(),
                ))?;
            column_writer
                .typed::<parquet::data_type::FixedLenByteArrayType>()
                .write_batch(&workspace.values_0000, None, None)?;
            column_writer.close()?;
            let mut column_writer = row_group_writer
                .next_column()?
                .ok_or_else(|| parquetry::error::Error::InvalidField(
                    "content".to_string(),
                ))?;
            column_writer
                .typed::<parquet::data_type::ByteArrayType>()
                .write_batch(&workspace.values_0001, None, None)?;
            column_writer.close()?;
            workspace.clear();
            Ok(row_group_writer.close()?)
        }
    }
    fn fill_workspace<
        'a,
        E: From<parquetry::error::Error>,
        I: Iterator<Item = Result<&'a Self, E>>,
    >(workspace: &mut ParquetryWorkspace, values: I) -> Result<usize, E> {
        {
            let mut written_count = 0;
            for result in values {
                Self::add_item_to_workspace(workspace, result?)?;
                written_count += 1;
            }
            Ok(written_count)
        }
    }
    fn add_item_to_workspace(
        workspace: &mut ParquetryWorkspace,
        value: &Self,
    ) -> Result<(), parquetry::error::Error> {
        {
            let Item { digest, content } = value;
            workspace.values_0000.push(digest.to_vec().into());
            workspace.values_0001.push(content.as_slice().into());
            Ok(())
        }
    }
}
impl Item {
    pub fn new(
        digest: [u8; 20],
        content: Vec<u8>,
    ) -> Result<Self, parquetry::error::ValueError> {
        Ok(Self { digest, content })
    }
}
#[derive(Default)]
struct ParquetryWorkspace {
    values_0000: Vec<parquet::data_type::FixedLenByteArray>,
    values_0001: Vec<parquet::data_type::ByteArray>,
}
impl ParquetryWorkspace {
    fn clear(&mut self) {
        self.values_0000.clear();
        self.values_0001.clear();
    }
}
#[cfg(test)]
mod test {
    impl quickcheck::Arbitrary for super::Item {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            Self::new(
                    [
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                        u8::arbitrary(g),
                    ],
                    <_>::arbitrary(g),
                )
                .expect("Invalid quickcheck::Arbitrary instance for Item")
        }
    }
    fn round_trip_write_impl(groups: Vec<Vec<super::Item>>) -> bool {
        let test_dir = tempdir::TempDir::new("Item-data").unwrap();
        let test_file_path = test_dir.path().join("write-data.parquet");
        let test_file = std::fs::File::create(&test_file_path).unwrap();
        <super::Item as parquetry::Schema>::write_row_groups(
                test_file,
                Default::default(),
                groups.clone(),
            )
            .unwrap();
        let read_file = std::fs::File::open(test_file_path).unwrap();
        let read_options = parquet::file::serialized_reader::ReadOptionsBuilder::new()
            .build();
        let read_values = <super::Item as parquetry::Schema>::read(
                read_file,
                read_options,
            )
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        read_values == groups.into_iter().flatten().collect::<Vec<_>>()
    }
    quickcheck::quickcheck! {
        fn round_trip_write(groups : Vec < Vec < super::Item >>) -> bool {
        round_trip_write_impl(groups) }
    }
    fn round_trip_serde_bincode_impl(values: Vec<super::Item>) -> bool {
        let wrapped = bincode::serde::Compat(&values);
        let encoded = bincode::encode_to_vec(&wrapped, bincode::config::standard())
            .unwrap();
        let decoded: (bincode::serde::Compat<Vec<super::Item>>, _) = bincode::decode_from_slice(
                &encoded.as_slice(),
                bincode::config::standard(),
            )
            .unwrap();
        decoded.0.0 == values
    }
    quickcheck::quickcheck! {
        fn round_trip_serde_bincode(values : Vec < super::Item >) -> bool {
        round_trip_serde_bincode_impl(values) }
    }
    fn gen_valid_timestamp_milli(g: &mut quickcheck::Gen) -> i64 {
        {
            use quickcheck::Arbitrary;
            let min = chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis();
            let max = chrono::DateTime::<chrono::Utc>::MAX_UTC.timestamp_millis();
            let value: i64 = <_>::arbitrary(g);
            if value < min {
                value % min
            } else if value > max {
                value % max
            } else {
                value
            }
        }
    }
    fn gen_valid_timestamp_micro(g: &mut quickcheck::Gen) -> i64 {
        {
            use quickcheck::Arbitrary;
            let min = chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_micros();
            let max = chrono::DateTime::<chrono::Utc>::MAX_UTC.timestamp_micros();
            let value: i64 = <_>::arbitrary(g);
            if value < min {
                value % min
            } else if value > max {
                value % max
            } else {
                value
            }
        }
    }
}

use parquet::{
    column::reader::ColumnReader,
    file::{
        reader::{ChunkReader, FileReader},
        serialized_reader::SerializedFileReader,
    },
};

pub mod item;
pub mod write;

pub fn read_parquet<R: ChunkReader + 'static>(
    file: R,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut total_read_count = 0;
    let reader = SerializedFileReader::new(file)?;

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

        total_read_count += digest_values.len();
    }

    Ok(total_read_count)
}

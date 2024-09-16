use parquet::{
    column::reader::ColumnReader,
    file::{
        reader::{ChunkReader, FileReader},
        serialized_reader::SerializedFileReader,
    },
};

pub mod item;

pub fn read_parquet<R: ChunkReader + 'static>(file: R) -> Result<(), Box<dyn std::error::Error>> {
    let reader = SerializedFileReader::new(file)?;

    let _metadata = reader.metadata();

    for index in 0..reader.num_row_groups() {
        let row_group_reader = reader.get_row_group(index)?;
        let _column_reader = match row_group_reader.get_column_reader(0)? {
            ColumnReader::FixedLenByteArrayColumnReader(reader) => reader,
            _ => panic!("Bad type"),
        };
    }

    Ok(())
}

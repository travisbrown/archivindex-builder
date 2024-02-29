use tantivy::schema::{Schema as TantivySchema, *};

pub const SNAPSHOT_ID_FIELD_NAME: &str = "snapshot_id";
pub const SURT_ID_FIELD_NAME: &str = "surt_id";
pub const PATTERN_FIELD_NAME: &str = "pattern";
pub const YEAR_FIELD_NAME: &str = "year";
pub const TIMESTAMP_FIELD_NAME: &str = "timestamp";
pub const CONTENT_FIELD_NAME: &str = "content";
pub const TITLE_FIELD_NAME: &str = "title";
pub const GRAVATAR_HASHES_FIELD_NAME: &str = "gravatar_hashes";

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Fields {
    pub snapshot_id: Field,
    pub surt_id: Field,
    pub pattern: Field,
    pub year: Field,
    pub timestamp: Field,
    pub content: Field,
    pub title: Field,
    pub gravatar_hashes: Field,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Schema {
    pub schema: TantivySchema,
    pub fields: Fields,
}

impl Default for Schema {
    fn default() -> Self {
        let content_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        Self::new(content_options)
    }
}

impl Schema {
    pub fn new(content_options: TextOptions) -> Self {
        let mut schema_builder = TantivySchema::builder();

        let snapshot_id_options = NumericOptions::default().set_indexed().set_stored();
        let surt_id_options = NumericOptions::default().set_indexed().set_stored();
        let pattern_options = FacetOptions::default().set_stored();
        let year_options = FacetOptions::default().set_stored();
        let timestamp_options = DateOptions::default()
            .set_indexed()
            .set_stored()
            .set_precision(DateTimePrecision::Seconds);

        let title_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        let gravatar_hashes_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::Basic),
            )
            .set_stored();

        let snapshot_id = schema_builder.add_i64_field(SNAPSHOT_ID_FIELD_NAME, snapshot_id_options);
        let surt_id = schema_builder.add_i64_field(SURT_ID_FIELD_NAME, surt_id_options);
        let pattern = schema_builder.add_facet_field(PATTERN_FIELD_NAME, pattern_options);
        let year = schema_builder.add_facet_field(YEAR_FIELD_NAME, year_options);
        let timestamp = schema_builder.add_date_field(TIMESTAMP_FIELD_NAME, timestamp_options);
        let content = schema_builder.add_text_field(CONTENT_FIELD_NAME, content_options);
        let title = schema_builder.add_text_field(TITLE_FIELD_NAME, title_options);
        let gravatar_hashes =
            schema_builder.add_text_field(GRAVATAR_HASHES_FIELD_NAME, gravatar_hashes_options);

        Self {
            schema: schema_builder.build(),
            fields: Fields {
                snapshot_id,
                surt_id,
                pattern,
                year,
                timestamp,
                content,
                title,
                gravatar_hashes,
            },
        }
    }
}

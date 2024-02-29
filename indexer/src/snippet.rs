use serde::ser::{Serialize, SerializeSeq, SerializeStruct, Serializer};
use std::ops::Range;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Snippet {
    fragment: String,
    highlighted: Vec<Range<usize>>,
}

impl Snippet {
    pub fn to_html(&self, tag: &str) -> String {
        let mut html = String::new();
        let mut start_from: usize = 0;

        for item in collapse_overlapped_ranges(&self.highlighted) {
            html.push_str(&html_escape::encode_text(
                &self.fragment[start_from..item.start],
            ));
            html.push_str(&format!("<{}>", tag));
            html.push_str(&html_escape::encode_text(&self.fragment[item.clone()]));
            html.push_str(&format!("</{}>", tag));
            start_from = item.end;
        }
        html.push_str(&html_escape::encode_text(
            &self.fragment[start_from..self.fragment.len()],
        ));
        html
    }
}

impl From<&tantivy::Snippet> for Snippet {
    fn from(value: &tantivy::Snippet) -> Self {
        Self {
            fragment: value.fragment().to_string(),
            highlighted: value.highlighted().to_vec(),
        }
    }
}

struct RangeWrapper<'a>(&'a Range<usize>);
struct RangeSeqWrapper<'a>(&'a Vec<Range<usize>>);

impl<'a> Serialize for RangeWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut range = serializer.serialize_seq(Some(2))?;
        range.serialize_element(&(self.0.start as u64))?;
        range.serialize_element(&(self.0.end as u64))?;
        range.end()
    }
}

impl<'a> Serialize for RangeSeqWrapper<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ranges = serializer.serialize_seq(Some(self.0.len()))?;

        for range in self.0 {
            ranges.serialize_element(&RangeWrapper(range))?;
        }

        ranges.end()
    }
}

impl Serialize for Snippet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut result = serializer.serialize_struct("Snippet", 2)?;
        result.serialize_field("fragment", &self.fragment)?;
        result.serialize_field("highlighted", &RangeSeqWrapper(&self.highlighted))?;
        result.end()
    }
}

// Borrowed from Tantivy.
fn collapse_overlapped_ranges(ranges: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut ranges_it = ranges.iter();

    let mut current = match ranges_it.next() {
        Some(range) => range.clone(),
        None => return result,
    };

    for range in ranges {
        if current.end > range.start {
            current = current.start..std::cmp::max(current.end, range.end);
        } else {
            result.push(current);
            current = range.clone();
        }
    }

    result.push(current);
    result
}

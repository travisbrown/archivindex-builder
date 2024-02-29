use super::top_computer::TopNComputer;
use super::Results;
use std::collections::HashMap;
use std::sync::Arc;
use tantivy::collector::Collector;
use tantivy::collector::SegmentCollector;
use tantivy::{DocAddress, DocId, Score, SegmentOrdinal, SegmentReader};

pub struct TopDocs {
    surt_map: Arc<HashMap<DocAddress, u64>>,
    pub limit: usize,
    pub offset: usize,
}

impl TopDocs {
    pub fn new(limit: usize, offset: usize, surt_map: Arc<HashMap<DocAddress, u64>>) -> Self {
        assert!(limit >= 1, "Limit must be strictly greater than 0.");
        Self {
            surt_map,
            limit,
            offset,
        }
    }
}

impl Collector for TopDocs {
    type Fruit = Results;
    type Child = TopScoreSegmentCollector;

    fn for_segment(
        &self,
        segment_local_id: SegmentOrdinal,
        _reader: &SegmentReader,
    ) -> tantivy::Result<Self::Child> {
        Ok(TopScoreSegmentCollector {
            surt_map: self.surt_map.clone(),
            all: HashMap::new(),
            top_n_computer: TopNComputer::new(self.limit + self.offset),
            segment_local_id,
        })
    }

    fn requires_scoring(&self) -> bool {
        true
    }

    fn merge_fruits(&self, child_fruits: Vec<Self::Fruit>) -> tantivy::Result<Self::Fruit> {
        Ok(if self.limit == 0 {
            Default::default()
        } else {
            Results::merge(child_fruits, self.limit + self.offset)
        })
    }
}

/// Segment Collector associated with `TopDocs`.
pub struct TopScoreSegmentCollector {
    surt_map: Arc<HashMap<DocAddress, u64>>,
    all: HashMap<u64, Vec<(Score, DocAddress)>>,
    top_n_computer: TopNComputer<Score, u64, false>,
    segment_local_id: u32,
}

impl SegmentCollector for TopScoreSegmentCollector {
    type Fruit = Results;

    fn collect(&mut self, doc_id: DocId, score: Score) {
        let doc_address = DocAddress {
            segment_ord: self.segment_local_id,
            doc_id,
        };
        let surt_id = self.surt_map.get(&doc_address).unwrap();
        self.top_n_computer.push(score, *surt_id);
        let entry = self.all.entry(*surt_id).or_default();
        entry.push((score, doc_address));
    }

    fn harvest(self) -> Self::Fruit {
        let top_n = self.top_n_computer.into_vec().into_iter().collect();

        Results {
            top_n,
            all: self.all,
        }
    }
}

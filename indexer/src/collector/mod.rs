use std::cmp::Reverse;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use tantivy::{DocAddress, Score};

mod top_computer;
mod top_score_collector;

pub use top_score_collector::TopDocs;

type ScoredSurt = (Score, (u64, Vec<(Score, DocAddress)>));

#[derive(Clone, Debug, Default)]
pub struct Results {
    top_n: Vec<top_computer::ComparableDoc<Score, u64>>,
    all: HashMap<u64, Vec<(Score, DocAddress)>>,
}

impl Results {
    pub fn top(mut self) -> Vec<ScoredSurt> {
        self.top_n
            .into_iter()
            .map(|top_computer::ComparableDoc { feature, doc }| {
                let mut snapshots = self.all.remove(&doc).unwrap();
                snapshots.sort_by(|(s1, d1), (s2, d2)| {
                    Reverse(*s1)
                        .partial_cmp(&Reverse(*s2))
                        .unwrap_or_else(|| d1.cmp(d2))
                });

                (feature, (doc, snapshots))
            })
            .collect()
    }

    pub fn merge(all_results: Vec<Self>, count: usize) -> Self {
        let mut result_all: HashMap<u64, Vec<(f32, DocAddress)>> = HashMap::new();
        let mut top_n_computer = top_computer::TopNComputer::new(count);

        for Self { top_n, all } in all_results {
            for top_computer::ComparableDoc { feature, doc } in top_n {
                // TODO: Try to avoid checking in cases where we know we haven't seen the SURT.
                top_n_computer.push_or_update(feature, doc);
            }

            for (surt_id, values) in all {
                let entry = result_all.entry(surt_id);
                match entry {
                    Entry::Occupied(mut entry) => {
                        entry.get_mut().extend(values);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(values);
                    }
                }
            }
        }

        Self {
            top_n: top_n_computer.into_sorted_vec(),
            all: result_all,
        }
    }
}

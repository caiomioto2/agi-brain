use anyhow::{Result, bail};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Node {
    pub id: i64,
    pub slug: String,
    #[serde(rename = "type", default = "default_type")]
    pub kind: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}
fn default_type() -> String {
    "note".into()
}
fn default_source() -> String {
    "default".into()
}

impl Node {
    pub fn key(&self) -> String {
        format!("{}:{}", self.source, self.slug)
    }
    pub fn display_title(&self) -> String {
        if !self.title.trim().is_empty() && self.title != self.slug {
            return self.title.clone();
        }
        let tail = self.slug.rsplit('/').next().unwrap_or(&self.slug);
        let tail = ["people-", "projects-", "companies-", "clients-"]
            .iter()
            .find_map(|p| tail.strip_prefix(p))
            .unwrap_or(tail);
        tail.split(['-', '_'])
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut chars = w.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
    pub fn day(&self) -> Option<NaiveDate> {
        parse_day(&self.created)
    }
}
pub fn parse_day(s: &str) -> Option<NaiveDate> {
    s.get(..10)
        .and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Link {
    pub source: i64,
    pub target: i64,
    #[serde(rename = "type", default = "default_link")]
    pub kind: String,
}
fn default_link() -> String {
    "mentions".into()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Payload {
    #[serde(default)]
    pub generated_at: String,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub generated_at: String,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub pairs: Vec<(usize, usize)>,
    pub neighbors: Vec<Vec<usize>>,
    pub by_id: HashMap<i64, usize>,
    pub degree: Vec<usize>,
    pub types: BTreeMap<String, usize>,
    pub sources: BTreeMap<String, usize>,
    pub dropped_links: usize,
    search: Vec<String>,
}
impl Graph {
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::from_payload(serde_json::from_slice(data)?)
    }
    pub fn demo() -> Self {
        Self::parse(include_bytes!("../assets/demo.json")).expect("bundled demo is valid")
    }
    pub fn from_payload(p: Payload) -> Result<Self> {
        let mut g = Self {
            generated_at: p.generated_at,
            nodes: p.nodes,
            ..Self::default()
        };
        let mut identities = HashSet::new();
        for (i, n) in g.nodes.iter().enumerate() {
            if g.by_id.insert(n.id, i).is_some() {
                bail!("Duplicate node ID {}", n.id);
            }
            if !identities.insert(n.key()) {
                bail!("Duplicate page {} in source {}", n.slug, n.source);
            }
            *g.types.entry(n.kind.clone()).or_default() += 1;
            *g.sources.entry(n.source.clone()).or_default() += 1;
            g.search.push(
                format!(
                    "{} {} {} {} {}",
                    n.display_title(),
                    n.slug,
                    n.kind,
                    n.source,
                    crate::locale::type_label(&n.kind)
                )
                .to_lowercase(),
            );
        }
        g.neighbors.resize(g.nodes.len(), Vec::new());
        g.degree.resize(g.nodes.len(), 0);
        let mut seen = HashSet::new();
        for l in p.links {
            let (Some(&s), Some(&t)) = (g.by_id.get(&l.source), g.by_id.get(&l.target)) else {
                g.dropped_links += 1;
                continue;
            };
            if s == t || !seen.insert((s.min(t), s.max(t))) {
                g.dropped_links += 1;
                continue;
            }
            g.pairs.push((s, t));
            g.links.push(l);
            g.neighbors[s].push(t);
            g.neighbors[t].push(s);
            g.degree[s] += 1;
            g.degree[t] += 1;
        }
        Ok(g)
    }
    pub fn search(&self, query: &str, limit: usize) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        let words: Vec<_> = query.split_whitespace().collect();
        let mut hits: Vec<_> = self
            .search
            .iter()
            .enumerate()
            .filter(|(_, s)| words.iter().all(|w| s.contains(w)))
            .map(|(i, _)| {
                let title = self.nodes[i].display_title().to_lowercase();
                let rank = if title == query {
                    0
                } else if title.starts_with(&query) {
                    1
                } else {
                    2
                };
                (rank, std::cmp::Reverse(self.degree[i]), i)
            })
            .collect();
        hits.sort_unstable();
        hits.into_iter().take(limit).map(|h| h.2).collect()
    }
    pub fn date_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        let mut dates = self.nodes.iter().filter_map(Node::day);
        let first = dates.next()?;
        Some(dates.fold((first, first), |(lo, hi), d| (lo.min(d), hi.max(d))))
    }
    pub fn cursor_day(&self, progress: f32) -> Option<NaiveDate> {
        let (lo, hi) = self.date_range()?;
        Some(
            lo + chrono::Duration::days(
                ((hi - lo).num_days() as f32 * progress.clamp(0., 1.)).round() as i64,
            ),
        )
    }
    pub fn histogram(&self, buckets: usize) -> Vec<usize> {
        let mut counts = vec![0; buckets];
        if buckets == 0 {
            return counts;
        }
        if let Some((lo, hi)) = self.date_range() {
            let days = (hi - lo).num_days().max(1) as f64;
            for d in self.nodes.iter().filter_map(Node::day) {
                let b = (((d - lo).num_days() as f64 / days) * (buckets - 1) as f64) as usize;
                counts[b.min(buckets - 1)] += 1;
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_edges_and_rejects_ambiguous_ids() {
        let mut p: Payload = serde_json::from_slice(include_bytes!("../assets/demo.json")).unwrap();
        let n = p.links.len();
        let l = p.links[0].clone();
        p.links.push(Link {
            source: l.target,
            target: l.source,
            kind: l.kind,
        });
        p.links.push(Link {
            source: -99,
            target: 1,
            kind: "bad".into(),
        });
        let g = Graph::from_payload(p.clone()).unwrap();
        assert_eq!(g.links.len(), n);
        assert_eq!(g.dropped_links, 2);
        p.nodes.push(p.nodes[0].clone());
        assert!(Graph::from_payload(p).is_err());
    }
    #[test]
    fn search_and_replay_follow_data() {
        let g = Graph::demo();
        let i = g.search("Ada Lovelace", 1)[0];
        assert_eq!(g.nodes[i].slug, "people/ada-lovelace");
        assert_eq!(g.search("pessoa Ada", 1), vec![i]);
        assert_eq!(g.search("person Ada", 1), vec![i]);
        let (lo, hi) = g.date_range().unwrap();
        assert_eq!(g.cursor_day(0.), Some(lo));
        assert_eq!(g.cursor_day(1.), Some(hi));
        assert_eq!(g.histogram(24).iter().sum::<usize>(), g.nodes.len());
        assert!(
            Graph::parse(br#"{"nodes":[],"links":[]}"#)
                .unwrap()
                .date_range()
                .is_none()
        );
    }
}

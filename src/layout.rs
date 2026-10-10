//! Barnes-Hut layout, adapted from Glia's MIT-licensed Swift engine.
//! Settles once in a worker. Existing pages stay pinned on incremental loads.
use crate::{
    graph::Graph,
    storage::{atomic_write, stable_hash},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y)
    }
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y)
    }
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s)
    }
    fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}
pub type Positions = HashMap<String, Point>;
pub fn load_cache(path: &Path) -> Positions {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Positions>(&b).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, p)| p.finite())
        .collect()
}
pub fn save_cache(path: &Path, g: &Graph, p: &[Point]) -> anyhow::Result<()> {
    let m: Positions = g.nodes.iter().zip(p).map(|(n, p)| (n.key(), *p)).collect();
    atomic_write(path, &serde_json::to_vec(&m)?)
}
pub fn seed(g: &Graph, placed: &Positions) -> Vec<Point> {
    let mut linked = 0;
    let mut orphan = 0;
    let ring = 60. + 8. * (g.degree.iter().filter(|d| **d > 0).count() as f32).sqrt();
    g.nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            if let Some(p) = placed.get(&n.key()) {
                return *p;
            }
            let jitter = (stable_hash(&n.key()) % 1000) as f32 / 1000. - 0.5;
            let near: Vec<_> = g.neighbors[i]
                .iter()
                .filter_map(|j| placed.get(&g.nodes[*j].key()))
                .collect();
            if !near.is_empty() {
                let a = (stable_hash(&n.key()) % 628) as f32 / 100.;
                return near
                    .iter()
                    .fold(Point::default(), |s, p| s.add(**p))
                    .mul(1. / near.len() as f32)
                    .add(Point::new(a.cos() * 9., a.sin() * 9.));
            }
            let (r, a) = if g.degree[i] > 0 {
                let rank = linked as f32;
                linked += 1;
                (
                    6. * rank.sqrt() + jitter * 3.,
                    2.3999631 * rank + jitter * 0.2,
                )
            } else {
                let rank = orphan as f32;
                orphan += 1;
                (ring * (1. + 0.35 * jitter), 2.3999631 * rank + jitter)
            };
            Point::new(r * a.cos(), r * a.sin())
        })
        .collect()
}

#[derive(Clone)]
struct Cell {
    origin: Point,
    size: f32,
    mass: f32,
    center: Point,
    body: Option<usize>,
    children: [Option<usize>; 4],
    leaf: bool,
}
impl Cell {
    fn new(origin: Point, size: f32) -> Self {
        Self {
            origin,
            size,
            mass: 0.,
            center: Point::default(),
            body: None,
            children: [None; 4],
            leaf: true,
        }
    }
}
struct Tree<'a> {
    cells: Vec<Cell>,
    positions: &'a [Point],
    masses: &'a [f32],
}
impl<'a> Tree<'a> {
    fn new(p: &'a [Point], m: &'a [f32]) -> Self {
        let mut lo = Point::new(f32::MAX, f32::MAX);
        let mut hi = Point::new(f32::MIN, f32::MIN);
        for v in p {
            lo.x = lo.x.min(v.x);
            lo.y = lo.y.min(v.y);
            hi.x = hi.x.max(v.x);
            hi.y = hi.y.max(v.y);
        }
        let mut t = Self {
            cells: Vec::with_capacity(p.len() * 3),
            positions: p,
            masses: m,
        };
        t.cells.push(Cell::new(
            lo,
            (hi.x - lo.x).max(hi.y - lo.y).max(1.) * 1.001,
        ));
        for i in 0..p.len() {
            t.insert(i, 0, 0);
        }
        t
    }
    fn insert(&mut self, body: usize, cell: usize, depth: u32) {
        let m = self.masses[body];
        let total = self.cells[cell].mass + m;
        self.cells[cell].center = self.cells[cell]
            .center
            .mul(self.cells[cell].mass)
            .add(self.positions[body].mul(m))
            .mul(1. / total);
        self.cells[cell].mass = total;
        if self.cells[cell].leaf {
            let Some(old) = self.cells[cell].body else {
                self.cells[cell].body = Some(body);
                return;
            };
            if depth >= 24 {
                return;
            }
            self.cells[cell].leaf = false;
            self.cells[cell].body = None;
            self.descend(old, cell, depth);
        }
        self.descend(body, cell, depth);
    }
    fn descend(&mut self, body: usize, cell: usize, depth: u32) {
        let o = self.cells[cell].origin;
        let h = self.cells[cell].size / 2.;
        let p = self.positions[body];
        let right = usize::from(p.x >= o.x + h);
        let top = usize::from(p.y >= o.y + h);
        let q = right + 2 * top;
        let child = if let Some(c) = self.cells[cell].children[q] {
            c
        } else {
            let c = self.cells.len();
            self.cells.push(Cell::new(
                Point::new(o.x + right as f32 * h, o.y + top as f32 * h),
                h,
            ));
            self.cells[cell].children[q] = Some(c);
            c
        };
        self.insert(body, child, depth + 1);
    }
    fn force(&self, at: Point, skip: usize) -> Point {
        let mut stack = vec![0];
        let mut result = Point::default();
        while let Some(i) = stack.pop() {
            let c = &self.cells[i];
            if c.mass == 0. || c.leaf && c.body == Some(skip) {
                continue;
            }
            let delta = at.sub(c.center);
            let d2 = delta.x * delta.x + delta.y * delta.y + 4.;
            let contains = at.x >= c.origin.x
                && at.x <= c.origin.x + c.size
                && at.y >= c.origin.y
                && at.y <= c.origin.y + c.size;
            if c.leaf || (!contains && c.size * c.size < 0.85 * 0.85 * d2) {
                result = result.add(delta.mul(c.mass / d2));
            } else {
                stack.extend(c.children.iter().flatten());
            }
        }
        result
    }
}
pub fn settle(g: &Graph, placed: &Positions, cancel: &AtomicBool) -> Vec<Point> {
    let mut p = seed(g, placed);
    if p.is_empty() {
        return p;
    }
    let pinned: Vec<_> = g
        .nodes
        .iter()
        .map(|n| placed.contains_key(&n.key()))
        .collect();
    if pinned.iter().all(|p| *p) {
        return p;
    }
    let mut v = vec![Point::default(); p.len()];
    let mass: Vec<_> = g.degree.iter().map(|d| (1 + *d) as f32).collect();
    let mut alpha = 1.;
    for _ in 0..260 {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let tree = Tree::new(&p, &mass);
        for i in 0..p.len() {
            if !pinned[i] {
                v[i] = v[i].add(tree.force(p[i], i).mul(22. * alpha));
            }
        }
        for &(s, t) in &g.pairs {
            let d = p[t].sub(p[s]);
            let dist = d.len().max(0.01);
            let f = d.mul((dist - 26.) / dist * 0.28 * alpha);
            let ws = mass[t] / (mass[s] + mass[t]);
            if !pinned[s] {
                v[s] = v[s].add(f.mul(ws));
            }
            if !pinned[t] {
                v[t] = v[t].sub(f.mul(1. - ws));
            }
        }
        for i in 0..p.len() {
            if !pinned[i] {
                v[i] = v[i].sub(p[i].mul(0.035 * alpha)).mul(0.82);
                p[i] = p[i].add(v[i]);
            }
        }
        alpha *= 0.9772;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_layout_and_insertions_preserve_the_map() {
        let g = Graph::demo();
        let stop = AtomicBool::new(false);
        let a = settle(&g, &Positions::new(), &stop);
        let b = settle(&g, &Positions::new(), &stop);
        assert_eq!(a, b);
        assert!(a.iter().all(|p| p.finite()));
        let placed: Positions = g
            .nodes
            .iter()
            .zip(&a)
            .take(100)
            .map(|(n, p)| (n.key(), *p))
            .collect();
        let c = settle(&g, &placed, &stop);
        assert_eq!(&a[..100], &c[..100]);
        let d = tempfile::tempdir().unwrap();
        let cache = d.path().join("layout.json");
        save_cache(&cache, &g, &a).unwrap();
        assert_eq!(settle(&g, &load_cache(&cache), &stop), a);
    }
}

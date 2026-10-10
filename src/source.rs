use crate::{
    graph::Graph,
    layout::{self, Point, Positions},
    storage::stable_hash,
};
use notify::{RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

pub enum Event {
    Checking,
    Current,
    Warning(String),
    Loaded {
        graph: Arc<Graph>,
        positions: Vec<Point>,
        new_keys: Vec<String>,
    },
    Settled {
        positions: Vec<Point>,
        warning: Option<String>,
    },
    Error(String),
    Page {
        key: String,
        result: Result<String, String>,
    },
    Online(String),
}
pub struct Worker {
    pub rx: mpsc::Receiver<Event>,
    stop: Arc<AtomicBool>,
    requests: mpsc::Sender<Request>,
}
enum Request {
    Changed,
    Refresh,
    Page(crate::graph::Node),
}
impl Worker {
    pub fn page(&self, node: crate::graph::Node) {
        let _ = self.requests.send(Request::Page(node));
    }
    pub fn refresh(&self) {
        let _ = self.requests.send(Request::Refresh);
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
pub fn read_graph(path: &Path) -> anyhow::Result<Graph> {
    let metadata = std::fs::metadata(path)?;
    anyhow::ensure!(metadata.len() <= 200_000_000, "O mapa ultrapassa 200 MB");
    Graph::parse(&std::fs::read(path)?)
}
pub fn spawn(path: Option<PathBuf>, cache: PathBuf, wake: Arc<dyn Fn() + Send + Sync>) -> Worker {
    let (tx, rx) = mpsc::channel();
    let (request_tx, change_rx) = mpsc::channel();
    let change_tx = request_tx.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let cancel = stop.clone();
    std::thread::spawn(move || {
        let mut watcher =
            notify::recommended_watcher(move |_event: notify::Result<notify::Event>| {
                let _ = change_tx.send(Request::Changed);
            })
            .ok();
        if let (Some(w), Some(p)) = (&mut watcher, &path)
            && let Some(parent) = p.parent()
        {
            let _ = w.watch(parent, RecursiveMode::NonRecursive);
        }
        let mut placed = layout::load_cache(&cache);
        let mut last_hash = None;
        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let parsed = if let Some(ref p) = path {
                std::fs::metadata(p)
                    .and_then(|m| {
                        if m.len() > 200_000_000 {
                            Err(std::io::Error::other("O mapa ultrapassa 200 MB"))
                        } else {
                            std::fs::read(p)
                        }
                    })
                    .map_err(anyhow::Error::from)
                    .and_then(|b| {
                        let hash = stable_hash(&String::from_utf8_lossy(&b));
                        if last_hash == Some(hash) {
                            return Ok(None);
                        }
                        Graph::parse(&b).map(|g| Some((g, hash)))
                    })
            } else {
                Ok(Some((Graph::demo(), 0)))
            };
            match parsed {
                Ok(Some((g, hash))) => {
                    last_hash = Some(hash);
                    let g = Arc::new(g);
                    let new_keys = g
                        .nodes
                        .iter()
                        .filter(|n| !placed.contains_key(&n.key()))
                        .map(|n| n.key())
                        .collect();
                    let start = layout::seed(&g, &placed);
                    if tx
                        .send(Event::Loaded {
                            graph: g.clone(),
                            positions: start,
                            new_keys,
                        })
                        .is_err()
                    {
                        break;
                    }
                    wake();
                    let positions = layout::settle(&g, &placed, &cancel);
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    placed = g
                        .nodes
                        .iter()
                        .zip(&positions)
                        .map(|(n, p)| (n.key(), *p))
                        .collect::<Positions>();
                    let warning = layout::save_cache(&cache, &g, &positions)
                        .err()
                        .map(|e| format!("As posições do mapa não foram salvas: {e}"));
                    if tx.send(Event::Settled { positions, warning }).is_err() {
                        break;
                    }
                    wake();
                }
                Ok(None) => {
                    let _ = tx.send(Event::Current);
                    wake();
                }
                Err(e) => {
                    let message = format!(
                        "{}: {e}",
                        path.as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    );
                    // A manual retry must finish even when the error is unchanged.
                    let _ = tx.send(Event::Error(message));
                    wake();
                }
            }
            if path.is_none() {
                break;
            }
            // Debounce from the LAST event, including replacement/rename bursts.
            // Hashing and parsing stay on this thread, never on the UI thread.
            for _ in 0..5 {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                match change_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(Request::Changed) => {
                        while change_rx.recv_timeout(Duration::from_millis(180)).is_ok() {
                            if cancel.load(Ordering::Relaxed) {
                                return;
                            }
                        }
                        break;
                    }
                    Ok(Request::Refresh) => break,
                    _ => {}
                }
            }
        }
    });
    Worker {
        rx,
        stop,
        requests: request_tx,
    }
}

pub fn spawn_live(
    host: String,
    interval: u64,
    cache: PathBuf,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> Worker {
    let (tx, rx) = mpsc::channel();
    let (request_tx, request_rx) = mpsc::channel::<Request>();
    let stop = Arc::new(AtomicBool::new(false));
    let cancel = stop.clone();
    std::thread::spawn(move || {
        let mut placed = layout::load_cache(&cache);
        let mut last_hash = None;
        let mut client = None;
        let graph_cache = crate::remote::cache_path(&host);
        // Metadata and positions only. Page bodies are read on demand, never cached to disk.
        if let Ok(g) = read_graph(&graph_cache) {
            last_hash = Some(crate::remote::fingerprint(&g));
            let graph = Arc::new(g);
            let positions = layout::seed(&graph, &placed);
            let _ = tx.send(Event::Loaded {
                graph: graph.clone(),
                positions: positions.clone(),
                new_keys: Vec::new(),
            });
            wake();
            let positions = layout::settle(&graph, &placed, &cancel);
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            placed = graph
                .nodes
                .iter()
                .zip(&positions)
                .map(|(n, p)| (n.key(), *p))
                .collect();
            let _ = layout::save_cache(&cache, &graph, &positions);
            let _ = tx.send(Event::Settled {
                positions,
                warning: None,
            });
            let _ = tx.send(Event::Checking);
            wake();
        }
        let mut next_poll = std::time::Instant::now();
        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if std::time::Instant::now() >= next_poll {
                let _ = tx.send(Event::Checking);
                wake();
                let result = (|| -> anyhow::Result<_> {
                    if client.is_none() {
                        client = Some(crate::remote::Client::connect(&host)?);
                    }
                    client.as_mut().unwrap().graph()
                })();
                match result {
                    Ok((g, bytes)) => {
                        let hash = crate::remote::fingerprint(&g);
                        if last_hash != Some(hash) {
                            last_hash = Some(hash);
                            let graph = Arc::new(g);
                            let new_keys = graph
                                .nodes
                                .iter()
                                .filter(|n| !placed.contains_key(&n.key()))
                                .map(|n| n.key())
                                .collect();
                            let positions = layout::seed(&graph, &placed);
                            if tx
                                .send(Event::Loaded {
                                    graph: graph.clone(),
                                    positions,
                                    new_keys,
                                })
                                .is_err()
                            {
                                break;
                            }
                            wake();
                            let positions = layout::settle(&graph, &placed, &cancel);
                            if cancel.load(Ordering::Relaxed) {
                                break;
                            }
                            placed = graph
                                .nodes
                                .iter()
                                .zip(&positions)
                                .map(|(n, p)| (n.key(), *p))
                                .collect();
                            let warning = layout::save_cache(&cache, &graph, &positions)
                                .err()
                                .map(|e| e.to_string());
                            let _ = tx.send(Event::Settled { positions, warning });
                        }
                        // A local cache failure must not report the AWS connection as offline.
                        if let Err(e) = crate::storage::atomic_write(&graph_cache, &bytes) {
                            let _ = tx.send(Event::Warning(format!(
                                "AWS conectada; não foi possível salvar a cópia local: {e}"
                            )));
                        }
                        let _ = tx.send(Event::Online(
                            chrono::Local::now().format("%H:%M:%S").to_string(),
                        ));
                        wake();
                    }
                    Err(e) => {
                        client = None;
                        let _ = tx.send(Event::Error(format!("AWS: {e}")));
                        wake();
                    }
                }
                next_poll = std::time::Instant::now() + Duration::from_secs(interval.clamp(5, 300));
            }
            match request_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Request::Refresh) => next_poll = std::time::Instant::now(),
                Ok(Request::Changed) => {}
                Ok(Request::Page(node)) => {
                    let key = node.key();
                    let result = (|| -> anyhow::Result<String> {
                        if client.is_none() {
                            client = Some(crate::remote::Client::connect(&host)?);
                        }
                        client
                            .as_mut()
                            .unwrap()
                            .pages(&[&node])?
                            .remove(&key)
                            .ok_or_else(|| {
                                anyhow::anyhow!("A página não existe mais no GBrain hospedado")
                            })
                    })()
                    .map_err(|e| e.to_string());
                    if result.is_err() {
                        client = None;
                    }
                    let _ = tx.send(Event::Page { key, result });
                    wake();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    Worker {
        rx,
        stop,
        requests: request_tx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait_loaded(worker: &Worker) -> (Arc<Graph>, Vec<Point>, Vec<String>) {
        loop {
            match worker.rx.recv_timeout(Duration::from_secs(10)).unwrap() {
                Event::Loaded {
                    graph,
                    positions,
                    new_keys,
                } => return (graph, positions, new_keys),
                Event::Error(e) => panic!("{e}"),
                _ => {}
            }
        }
    }
    fn wait_settled(worker: &Worker) -> Vec<Point> {
        loop {
            match worker.rx.recv_timeout(Duration::from_secs(10)).unwrap() {
                Event::Settled { positions, .. } => return positions,
                Event::Error(e) => panic!("{e}"),
                _ => {}
            }
        }
    }
    #[test]
    fn burst_updates_wait_for_quiet_preserve_positions_and_remove_missing_pages() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("graph.json");
        let initial = br#"{"nodes":[{"id":1,"slug":"old"}],"links":[]}"#;
        std::fs::write(&p, initial).unwrap();
        let worker = spawn(
            Some(p.clone()),
            d.path().join("layout/cache.json"),
            Arc::new(|| {}),
        );
        let _ = wait_loaded(&worker);
        let original = wait_settled(&worker);
        for _ in 0..4 {
            crate::storage::atomic_write(
                &p,
                br#"{"nodes":[{"id":1,"slug":"old"},{"id":2,"slug":"temporary"}],"links":[]}"#,
            )
            .unwrap();
            std::thread::sleep(Duration::from_millis(70));
        }
        crate::storage::atomic_write(&p, br#"{"nodes":[{"id":1,"slug":"old"},{"id":3,"slug":"new"}],"links":[{"source":1,"target":3}]}"#).unwrap();
        let (g, start, new_keys) = wait_loaded(&worker);
        assert_eq!(
            g.nodes[1].slug, "new",
            "intermediate exports must not appear"
        );
        assert_eq!(new_keys, vec!["default:new"]);
        assert_eq!((start[0].x, start[0].y), (original[0].x, original[0].y));
        assert!(
            (start[1].x - start[0].x).hypot(start[1].y - start[0].y) <= 10.,
            "new page starts beside its known neighbor"
        );
        let _ = wait_settled(&worker);
        crate::storage::atomic_write(&p, initial).unwrap();
        let (g, _, _) = wait_loaded(&worker);
        assert_eq!(g.nodes.len(), 1);
        assert!(g.links.is_empty());
        let _ = wait_settled(&worker);
        worker.refresh();
        loop {
            match worker.rx.recv_timeout(Duration::from_secs(2)).unwrap() {
                Event::Current => break,
                Event::Loaded { .. } => panic!("unchanged graph must not restart its animation"),
                Event::Error(e) => panic!("{e}"),
                _ => {}
            }
        }
    }
    #[test]
    fn watcher_retries_invalid_export_and_detects_same_stamp_replacement() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("graph.json");
        std::fs::write(&p, br#"{"generated_at":"same","nodes":[],"links":[]}"#).unwrap();
        let w = spawn(
            Some(p.clone()),
            d.path().join("cache/layout.json"),
            Arc::new(|| {}),
        );
        assert!(matches!(
            w.rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Event::Loaded { .. }
        ));
        assert!(matches!(
            w.rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Event::Settled { .. }
        ));
        std::fs::write(&p, "partial").unwrap();
        assert!(matches!(
            w.rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Event::Error(_)
        ));
        w.refresh();
        assert!(matches!(
            w.rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Event::Error(_)
        ));
        crate::storage::atomic_write(
            &p,
            br#"{"generated_at":"same","nodes":[{"id":1,"slug":"hello"}],"links":[]}"#,
        )
        .unwrap();
        let event = w.rx.recv_timeout(Duration::from_secs(10)).unwrap();
        if let Event::Loaded { graph, .. } = event {
            assert_eq!(graph.nodes.len(), 1);
        } else {
            panic!("expected valid replacement");
        }
    }
}

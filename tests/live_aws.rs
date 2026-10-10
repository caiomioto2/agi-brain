//! Opt-in end-to-end checks against the existing private AWS reader, no writes.
use glia::{
    graph::Graph,
    remote::Client,
    source::{self, Event},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires the user's configured AWS SSH host and deployed read-only bridge"]
fn aws_graph_pages_protocol_and_polling_are_functional() {
    let host = std::env::var("GLIA_TEST_SSH_HOST")
        .expect("Set GLIA_TEST_SSH_HOST to your configured SSH alias before running this test");
    let mut c = Client::connect(&host).unwrap();
    let (g, bytes) = c.graph().unwrap();
    assert!(!g.nodes.is_empty());
    assert!(!g.links.is_empty());
    assert_eq!(Graph::parse(&bytes).unwrap().nodes.len(), g.nodes.len());
    let n = g.nodes.iter().find(|n| n.source == "default").unwrap();
    let pages = c.pages(&[n]).unwrap();
    assert!(!pages[&n.key()].trim().is_empty());
    assert!(
        c.request(serde_json::json!({"op":"sql","sql":"SELECT 1"}))
            .is_err()
    );
    assert_eq!(
        c.request(serde_json::json!({"op":"health"})).unwrap()["read_only"],
        "on"
    );
    let (_, next) = c.graph().unwrap();
    let next = Graph::parse(&next).unwrap();
    assert!(next.nodes.len() >= g.nodes.len().saturating_sub(10));
    drop(c);
    let d = tempfile::tempdir().unwrap();
    let worker = source::spawn_live(host, 5, d.path().join("layout.json"), Arc::new(|| {}));
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut online = 0;
    let mut loaded = false;
    let mut settled = false;
    while Instant::now() < deadline && online < 2 {
        match worker.rx.recv_timeout(Duration::from_secs(40)).unwrap() {
            Event::Loaded { graph, .. } => {
                assert!(!graph.nodes.is_empty());
                loaded = true;
            }
            Event::Settled { positions, .. } => {
                assert!(positions.iter().all(|p| p.finite()));
                settled = true;
            }
            Event::Online(_) => online += 1,
            Event::Error(e) if !e.contains("cached") => {
                panic!("{e}");
            }
            _ => {}
        }
    }
    assert!(loaded && settled);
    assert_eq!(online, 2);
    // Force only this test's SSH reader to exit; never touch the AWS service
    // or another application's connection. The worker must recover on its own.
    #[cfg(windows)]
    {
        let command = format!(
            "$ErrorActionPreference='Stop'; $owned = @(Get-CimInstance Win32_Process -Filter 'ParentProcessId={}' | Where-Object {{ $_.Name -eq 'ssh.exe' -and $_.CommandLine -like '*read_only_bridge.py*' }}); if ($owned.Count -ne 1) {{ throw 'Expected exactly one owned reader' }}; Stop-Process -Id $owned[0].ProcessId",
            std::process::id()
        );
        let status = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &command])
            .status()
            .unwrap();
        assert!(status.success());
        worker.refresh();
        let deadline = Instant::now() + Duration::from_secs(50);
        let mut interrupted = false;
        let mut recovered = false;
        while Instant::now() < deadline {
            match worker.rx.recv_timeout(Duration::from_secs(40)).unwrap() {
                Event::Error(_) => interrupted = true,
                Event::Online(_) if interrupted => {
                    recovered = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(
            interrupted && recovered,
            "reader must reconnect without a UI restart"
        );
        worker.page(n.clone());
        loop {
            if let Event::Page { key, result } =
                worker.rx.recv_timeout(Duration::from_secs(40)).unwrap()
            {
                assert_eq!(key, n.key());
                assert!(!result.unwrap().trim().is_empty());
                break;
            }
        }
    }
    println!(
        "AWS verified: {} pages, {} links, {} sources, page read, denied SQL, two live polls, interrupted reader recovered",
        g.nodes.len(),
        g.links.len(),
        g.sources.len()
    );
}

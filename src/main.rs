#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use glia::{
    app::{GliaApp, Launch},
    graph::Graph,
    layout, source, storage,
};
use std::sync::atomic::AtomicBool;

fn main() {
    if let Err(error) = run() {
        eprintln!("AGI Brain: {error:#}");
        let _ = std::fs::create_dir_all(storage::state_dir());
        let _ = std::fs::write(
            storage::state_dir().join("last-error.log"),
            format!("{error:#}"),
        );
        std::process::exit(1);
    }
}
fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--check") {
        let p = args
            .get(1)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(storage::default_graph);
        let g = source::read_graph(&storage::resolve_graph(&p))?;
        println!(
            "{}",
            serde_json::json!({"pages":g.nodes.len(),"links":g.links.len(),"dropped_links":g.dropped_links,"types":g.types,"sources":g.sources})
        );
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--benchmark") {
        let n = args
            .get(1)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(6600)
            .clamp(1, 100000);
        let nodes = (0..n)
            .map(|i| serde_json::json!({"id":i,"slug":format!("notes/page-{i}"),"type":"note"}))
            .collect::<Vec<_>>();
        let links = (1..n)
            .map(|i| serde_json::json!({"source":i,"target":i/5,"type":"mentions"}))
            .collect::<Vec<_>>();
        let g = Graph::parse(&serde_json::to_vec(
            &serde_json::json!({"nodes":nodes,"links":links}),
        )?)?;
        let start = std::time::Instant::now();
        let p = layout::settle(&g, &Default::default(), &AtomicBool::new(false));
        println!(
            "{}",
            serde_json::json!({"pages":n,"links":g.links.len(),"layout_ms":start.elapsed().as_millis(),"finite":p.iter().all(|p|p.finite())})
        );
        return Ok(());
    }
    if args.iter().any(|a| a == "--help") {
        println!(
            "AGI Brain para Windows\n  agi-brain [--demo] [--data ARQUIVO_OU_PASTA] [--live SERVIDOR_SSH]\n  agi-brain --check ARQUIVO\n  agi-brain --benchmark [TOTAL_DE_PÁGINAS]\nCaptura: GLIA_SNAPSHOT=out.png, GLIA_SNAPSHOT_FOCUS=identificador, GLIA_SNAPSHOT_REPLAY=0.3"
        );
        return Ok(());
    }
    let mut launch = Launch::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--demo" => launch.demo = true,
            "--live" => {
                i += 1;
                launch.live = Some(
                    args.get(i)
                        .ok_or_else(|| {
                            anyhow::anyhow!("--live precisa do nome de um servidor SSH configurado")
                        })?
                        .clone(),
                );
            }
            "--data" => {
                i += 1;
                launch.path = Some(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--data precisa de um arquivo ou pasta"))?
                        .into(),
                );
            }
            s => anyhow::bail!("Argumento desconhecido: {s}"),
        };
        i += 1;
    }
    launch.snapshot = std::env::var_os("GLIA_SNAPSHOT").map(Into::into);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("AGI Brain — seu GBrain")
            .with_inner_size([1360., 900.])
            .with_min_inner_size([800., 580.])
            .with_icon(glia::canvas::icon()),
        renderer: eframe::Renderer::Wgpu,
        persistence_path: Some(storage::state_dir().join("window.ron")),
        ..Default::default()
    };
    eframe::run_native(
        "AGI Brain",
        options,
        Box::new(move |cc| Ok(Box::new(GliaApp::new(cc, launch)))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

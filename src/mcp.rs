//! Read-only native MCP companion. No client registration, shell execution or brain writes.
use crate::{
    context::{self, Scope},
    graph::Graph,
    source,
    storage::{self, Settings},
};
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};

pub fn tools() -> Value {
    json!([
        {"name":"health","description":"Check the connected graph, page access and explicitly exported psyche.","inputSchema":{"type":"object","properties":{}}},
        {"name":"who_am_i","description":"Read the explicitly exported psyche identity file.","inputSchema":{"type":"object","properties":{"max_tokens":{"type":"integer","minimum":100,"maximum":250000}}}},
        {"name":"recall","description":"Search graph metadata lexically and read matching pages from AWS or local folders.","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"max_tokens":{"type":"integer"},"top_k":{"type":"integer"}},"required":["query"]}},
        {"name":"prime_context","description":"Combine explicitly exported identity with matching pages from the connected brain.","inputSchema":{"type":"object","properties":{"task":{"type":"string"},"mode":{"type":"string","enum":["psyche","context","both"]},"max_tokens":{"type":"integer"}},"required":["task"]}},
        {"name":"explain_context","description":"Preview identity and retrieval status without returning page bodies.","inputSchema":{"type":"object","properties":{"task":{"type":"string"}},"required":["task"]}}
    ])
}
fn budget(a: &Value) -> usize {
    a.get("max_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(8000)
        .clamp(100, 250000) as usize
}
fn cut(s: &str, tokens: usize) -> String {
    s.chars().take(tokens * 4).collect()
}
fn psyche(tokens: usize) -> anyhow::Result<String> {
    let p = storage::psyche_path();
    anyhow::ensure!(
        std::fs::metadata(&p)?.len() <= 4_000_000,
        "Psyche is larger than 4 MB"
    );
    let text = std::fs::read_to_string(p)?;
    anyhow::ensure!(
        !text.trim().is_empty(),
        "Exporte a identidade pelo AGI Brain primeiro"
    );
    Ok(cut(&text, tokens))
}
fn data() -> anyhow::Result<(Graph, Settings, std::path::PathBuf)> {
    let mut settings = Settings::load(&storage::state_dir().join("settings.json"))?;
    if std::env::var_os("GLIA_DATA").is_some() {
        settings.live = false;
    }
    if settings.live {
        let mut client = crate::remote::Client::connect(&settings.ssh_host)?;
        let (graph, _) = client.graph()?;
        let path = crate::remote::cache_path(&settings.ssh_host);
        return Ok((graph, settings, path));
    }
    let path = std::env::var_os("GLIA_DATA")
        .map(std::path::PathBuf::from)
        .or_else(|| settings.graph_path.clone())
        .unwrap_or_else(storage::default_graph);
    let path = storage::resolve_graph(&path);
    Ok((source::read_graph(&path)?, settings, path))
}
fn recall(query: &str, args: &Value, tokens: usize) -> anyhow::Result<String> {
    let (g, s, path) = data()?;
    let top = args
        .get("top_k")
        .and_then(Value::as_u64)
        .unwrap_or(8)
        .clamp(1, 100) as usize;
    let hits = g.search(query, top);
    let keys = hits.iter().map(|i| g.nodes[*i].key()).collect();
    let b = context::build(&g, &s, Some(&path), Scope::Collection, None, &keys, tokens);
    if let Some(error) = b.error {
        anyhow::bail!("{error}");
    }
    anyhow::ensure!(
        hits.is_empty() || b.pages > 0,
        "Matching pages were found but their content is unavailable"
    );
    Ok(format!(
        "> Lexical graph retrieval: {} matches, {} readable pages, {} unavailable pages, budget capped: {}.\n\n{}",
        hits.len(),
        b.pages,
        b.missing,
        b.capped,
        b.text
    ))
}
pub fn call(name: &str, args: &Value) -> anyhow::Result<String> {
    let tokens = budget(args);
    match name{
        "health"=>Ok(json!({"server":"agi-brain-context-rust","read_only":true,"retrieval":"graph lexical",
            "graph":data().map(|(g,_,p)|json!({"path":p,"pages":g.nodes.len(),"links":g.links.len()})).unwrap_or_else(|e|json!({"error":e.to_string()})),
            "psyche_path":storage::psyche_path(),"psyche_readable":psyche(100).is_ok()}).to_string()),
        "who_am_i"=>psyche(tokens),
        "recall"=>recall(args.get("query").and_then(Value::as_str).ok_or_else(||anyhow::anyhow!("query is required"))?,args,tokens),
        "prime_context"=>{
            let task=args.get("task").and_then(Value::as_str).ok_or_else(||anyhow::anyhow!("task is required"))?;
            let mode=args.get("mode").and_then(Value::as_str).unwrap_or("both");
            anyhow::ensure!(["both","context","psyche"].contains(&mode),"Invalid mode");
            let mut out=String::new();let mut status=Vec::new();
            if mode!="context"{match psyche(if mode=="both"{tokens*2/5}else{tokens}){Ok(s)=>out.push_str(&s),Err(e)=>status.push(format!("identity unavailable: {e}"))}}
            if mode!="psyche"{let remaining=tokens.saturating_sub(out.chars().count().div_ceil(4));match recall(task,args,remaining){Ok(s)=>{out.push_str("\n\n");out.push_str(&s)},Err(e)=>status.push(format!("retrieval unavailable: {e}"))}}
            Ok(format!("> agi-brain-context status: {} — read-only graph retrieval{}\n\n{}",if status.is_empty(){"OK"}else{"DEGRADED"},if status.is_empty(){String::new()}else{format!("; {}",status.join("; "))},cut(&out,tokens)))
        }
        "explain_context"=>{
            let task=args.get("task").and_then(Value::as_str).unwrap_or("");
            let pages=data().map(|(g,_,_)|g.search(task,8).iter().map(|i|g.nodes[*i].key()).collect::<Vec<_>>()).unwrap_or_default();
            Ok(json!({"identity":"explicit psyche export","psyche_readable":psyche(100).is_ok(),"retrieval":"graph lexical","matched_pages":pages,"max_tokens":tokens}).to_string())
        }
        _=>anyhow::bail!("Unknown tool: {name}"),
    }
}
pub fn handle(request: &Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = &request["params"];
    let result = match method {
        "initialize" => {
            let requested = params["protocolVersion"].as_str().unwrap_or("2024-11-05");
            let version =
                if ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"].contains(&requested) {
                    requested
                } else {
                    "2025-06-18"
                };
            json!({"protocolVersion":version,"capabilities":{"tools":{}},"serverInfo":{"name":"agi-brain-context-rust","version":env!("CARGO_PKG_VERSION")},"instructions":"Assistente de contexto AGI Brain, somente leitura. Use prime_context for explicit identity exports and matching pages from the connected brain. Report DEGRADED status to the user."})
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools":tools()}),
        "tools/call" => match call(params["name"].as_str().unwrap_or(""), &params["arguments"]) {
            Ok(text) => json!({"content":[{"type":"text","text":text}],"isError":false}),
            Err(e) => json!({"content":[{"type":"text","text":e.to_string()}],"isError":true}),
        },
        _ => {
            return Some(
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}}),
            );
        }
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}
pub fn serve(input: impl Read, mut output: impl Write) -> anyhow::Result<()> {
    let mut input = std::io::BufReader::new(input);
    loop {
        let mut bytes = Vec::new();
        let n = input
            .by_ref()
            .take(1_048_577)
            .read_until(b'\n', &mut bytes)?;
        if n == 0 {
            break;
        }
        anyhow::ensure!(n <= 1_048_576, "MCP message exceeds 1 MB");
        let reply = match serde_json::from_slice::<Value>(&bytes) {
            Ok(request) => handle(&request),
            Err(_) => Some(
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}),
            ),
        };
        if let Some(reply) = reply {
            serde_json::to_writer(&mut output, &reply)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stdio_negotiates_lists_tools_and_reports_failures_without_protocol_noise() {
        let input=b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-03-26\"}}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"id\":2,\"method\":\"tools/list\"}\n{\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"unknown\"}}\n";
        let mut out = Vec::new();
        serve(&input[..], &mut out).unwrap();
        let replies: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(replies.len(), 3);
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(replies[1]["result"]["tools"].as_array().unwrap().len(), 5);
        assert_eq!(replies[2]["result"]["isError"], true);
    }
}

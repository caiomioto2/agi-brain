fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(flag) = args.get(1) {
        let name = match flag.as_str() {
            "--health" => "health",
            "--prime" => "prime_context",
            "--explain" => "explain_context",
            _ => anyhow::bail!("Uso: agi-brain-context [--health | --prime TASK | --explain TASK]"),
        };
        let value = glia::mcp::call(
            name,
            &serde_json::json!({"task":args.get(2).cloned().unwrap_or_default()}),
        )?;
        println!("{value}");
        Ok(())
    } else {
        glia::mcp::serve(std::io::stdin().lock(), std::io::stdout().lock())
    }
}

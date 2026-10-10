//! Private, read-only hosted graph protocol over the user's existing OpenSSH.
use crate::{
    graph::{Graph, Node},
    storage,
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

const READER: &str = "set -a; . /root/.gbrain/gbrain-mcp.env; set +a; exec python3 -u /root/.local/share/glia/read_only_bridge.py";
pub fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 101
        && host.as_bytes()[0].is_ascii_alphanumeric()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
pub fn cache_path(host: &str) -> std::path::PathBuf {
    storage::state_dir()
        .join("remote")
        .join(format!("{:016x}.json", storage::stable_hash(host)))
}
pub struct Client {
    child: Child,
    input: ChildStdin,
    replies: mpsc::Receiver<Result<Value>>,
    serial: u64,
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Client {
    pub fn connect(host: &str) -> Result<Self> {
        anyhow::ensure!(
            valid_host(host),
            "Informe um servidor SSH válido já configurado"
        );
        let mut cmd = Command::new("ssh");
        cmd.args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=2",
            host,
            READER,
        ]);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd.spawn().context("O OpenSSH não está disponível")?;
        let input = child
            .stdin
            .take()
            .context("Não foi possível abrir a entrada SSH")?;
        let stdout = child
            .stdout
            .take()
            .context("Não foi possível abrir a saída SSH")?;
        let (tx, replies) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            loop {
                let mut bytes = Vec::new();
                let reply = (|| -> Result<Value> {
                    let n = reader
                        .by_ref()
                        .take(200_000_001)
                        .read_until(b'\n', &mut bytes)?;
                    anyhow::ensure!(
                        n > 0,
                        "A conexão SSH com a AWS foi encerrada; verifique o servidor, o acesso e a instalação do leitor"
                    );
                    anyhow::ensure!(n <= 200_000_000, "O mapa hospedado ultrapassa 200 MB");
                    serde_json::from_slice(&bytes).context("Resposta inválida do leitor hospedado")
                })();
                let failed = reply.is_err();
                if tx.send(reply).is_err() || failed {
                    break;
                }
            }
        });
        let mut client = Self {
            child,
            input,
            replies,
            serial: 0,
        };
        let health = client.request(json!({"op":"health"}))?;
        anyhow::ensure!(
            health["read_only"] == "on" && health["protocol"] == 1,
            "O leitor hospedado deve garantir acesso somente para leitura"
        );
        Ok(client)
    }
    pub fn request(&mut self, mut request: Value) -> Result<Value> {
        self.serial += 1;
        request["id"] = json!(self.serial);
        serde_json::to_writer(&mut self.input, &request)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        let reply = self
            .replies
            .recv_timeout(Duration::from_secs(35))
            .context("O leitor da AWS excedeu o tempo de espera")??;
        anyhow::ensure!(
            reply["id"] == self.serial,
            "Identificador inesperado na resposta hospedada"
        );
        if let Some(error) = reply.get("error") {
            bail!("{}", error.as_str().unwrap_or("Falha na leitura hospedada"));
        }
        reply
            .get("result")
            .cloned()
            .context("Resultado hospedado ausente")
    }
    pub fn graph(&mut self) -> Result<(Graph, Vec<u8>)> {
        let value = self.request(json!({"op":"graph"}))?;
        let bytes = serde_json::to_vec(&value)?;
        Ok((Graph::parse(&bytes)?, bytes))
    }
    pub fn pages(&mut self, nodes: &[&Node]) -> Result<std::collections::HashMap<String, String>> {
        if nodes.is_empty() {
            return Ok(Default::default());
        }
        let keys = nodes
            .iter()
            .map(|n| json!({"source":n.source,"slug":n.slug}))
            .collect::<Vec<_>>();
        let response = self.request(json!({"op":"pages","keys":keys}))?;
        let rows = response
            .as_array()
            .context("Resposta inválida da página hospedada")?;
        Ok(rows
            .iter()
            .filter_map(|p| {
                Some((
                    format!("{}:{}", p["source"].as_str()?, p["slug"].as_str()?),
                    p["body"].as_str()?.to_owned(),
                ))
            })
            .collect())
    }
}
pub fn fingerprint(graph: &Graph) -> u64 {
    storage::stable_hash(&serde_json::to_string(&(&graph.nodes, &graph.links)).unwrap_or_default())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ssh_aliases_cannot_inject_options_or_commands() {
        assert!(valid_host("gbrain-aws"));
        assert!(valid_host("gbrain.example.com"));
        for h in [
            "",
            "-oProxyCommand=bad",
            "host; command",
            "root@host",
            "host\ncommand",
            "host bad",
        ] {
            assert!(!valid_host(h));
        }
    }
    #[test]
    fn timestamps_do_not_trigger_spurious_graph_updates() {
        let a = Graph::parse(br#"{"generated_at":"a","nodes":[],"links":[]}"#).unwrap();
        let b = Graph::parse(br#"{"generated_at":"b","nodes":[],"links":[]}"#).unwrap();
        assert_eq!(fingerprint(&a), fingerprint(&b));
        let c = Graph::parse(br#"{"generated_at":"b","nodes":[{"id":1,"slug":"new"}],"links":[]}"#)
            .unwrap();
        assert_ne!(fingerprint(&a), fingerprint(&c));
    }
}

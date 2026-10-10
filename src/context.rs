use crate::{
    graph::{Graph, Node},
    storage::{self, Settings},
};
use anyhow::{Result, bail};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Collection,
    Selection,
    Identity,
    Everything,
}
impl Scope {
    pub const ALL: [Self; 4] = [
        Self::Collection,
        Self::Selection,
        Self::Identity,
        Self::Everything,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Collection => "Meus favoritos",
            Self::Selection => "Seleção e conexões",
            Self::Identity => "Mapa de identidade",
            Self::Everything => "Todas as páginas disponíveis",
        }
    }
}
#[derive(Default)]
pub struct Bundle {
    pub text: String,
    pub pages: usize,
    pub missing: usize,
    pub capped: bool,
    pub error: Option<String>,
}
impl Bundle {
    pub fn tokens(&self) -> usize {
        self.text.chars().count().div_ceil(4)
    }
}
pub fn mirror(settings: &Settings, graph_path: Option<&Path>, source: &str) -> Option<PathBuf> {
    if source.is_empty()
        || !source
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    settings
        .mirrors
        .get(source)
        .cloned()
        .or_else(|| storage::brain_home(graph_path?).map(|h| h.join(format!("source-{source}"))))
}
/// Validate both lexical components and resolved paths (including Windows junctions).
pub fn page_path(base: &Path, slug: &str) -> Result<PathBuf> {
    if slug.is_empty()
        || slug.contains(['\\', ':', '\0'])
        || slug.starts_with('/')
        || slug
            .split('/')
            .any(|p| p == ".." || p == "." || p.is_empty())
    {
        bail!("Caminho de página inválido");
    }
    let root = std::fs::canonicalize(base)?;
    let path = std::fs::canonicalize(root.join(format!("{slug}.md")))?;
    if !path.starts_with(&root) {
        bail!("A página está fora da pasta da fonte");
    }
    Ok(path)
}
pub fn read_page(settings: &Settings, graph_path: Option<&Path>, node: &Node) -> Result<String> {
    let base = mirror(settings, graph_path, &node.source)
        .ok_or_else(|| anyhow::anyhow!("Nenhuma pasta local para a fonte '{}'", node.source))?;
    let path = page_path(&base, &node.slug)?;
    if std::fs::metadata(&path)?.len() > 400_000 {
        bail!("A página ultrapassa 400 KB");
    }
    Ok(strip_frontmatter(&std::fs::read_to_string(path)?).to_owned())
}
pub fn strip_frontmatter(text: &str) -> &str {
    let text = text.trim_start_matches('\u{feff}');
    if let Some(rest) = text
        .strip_prefix("---\r\n")
        .or_else(|| text.strip_prefix("---\n"))
    {
        let mut offset = 0;
        for line in rest.split_inclusive('\n') {
            offset += line.len();
            if line.trim() == "---" {
                return rest[offset..].trim();
            }
        }
    }
    text.trim()
}
fn rank(n: &Node, self_slug: &str) -> u8 {
    if !self_slug.is_empty() && n.slug == self_slug {
        0
    } else if n.kind == "original" {
        1
    } else if n.kind == "concept" {
        2
    } else if n.kind == "person" || n.kind == "company" {
        3
    } else {
        4
    }
}
pub fn build(
    g: &Graph,
    settings: &Settings,
    graph_path: Option<&Path>,
    scope: Scope,
    selected: Option<usize>,
    starred: &std::collections::BTreeSet<String>,
    max_tokens: usize,
) -> Bundle {
    let mut indices: Vec<usize> = match scope {
        Scope::Selection => selected
            .map(|i| {
                std::iter::once(i)
                    .chain(g.neighbors[i].iter().copied())
                    .collect()
            })
            .unwrap_or_default(),
        _ => (0..g.nodes.len())
            .filter(|i| match scope {
                Scope::Collection => starred.contains(&g.nodes[*i].key()),
                Scope::Identity => [
                    "person", "company", "concept", "original", "note", "meeting",
                ]
                .contains(&g.nodes[*i].kind.as_str()),
                _ => true,
            })
            .collect(),
    };
    indices.sort_by_key(|i| (rank(&g.nodes[*i], &settings.self_slug), g.nodes[*i].key()));
    let mut out = Bundle {
        text: format!(
            "# AGI Brain — {}\n<!-- Exportação explícita, somente leitura. -->\n\n",
            scope.label()
        ),
        ..Bundle::default()
    };
    let mut seen = HashSet::new();
    let budget = max_tokens.clamp(100, 250_000) * 4;
    let mut hosted = if settings.live {
        match crate::remote::Client::connect(&settings.ssh_host) {
            Ok(c) => Some(c),
            Err(e) => {
                out.error = Some(e.to_string());
                return out;
            }
        }
    } else {
        None
    };
    let mut hosted_pages = std::collections::HashMap::new();
    for (offset, i) in indices.iter().copied().enumerate() {
        let n = &g.nodes[i];
        if !seen.insert(n.key()) {
            continue;
        }
        let body = if settings.live {
            if offset % 100 == 0
                && let Some(client) = &mut hosted
            {
                // Fetch bounded batches once; budget caps avoid reading the whole brain.
                let candidates = indices[offset..(offset + 100).min(indices.len())]
                    .iter()
                    .map(|i| &g.nodes[*i])
                    .collect::<Vec<_>>();
                match client.pages(&candidates) {
                    Ok(pages) => hosted_pages = pages,
                    Err(e) => {
                        out.error = Some(e.to_string());
                        break;
                    }
                }
            }
            hosted_pages.remove(&n.key()).ok_or_else(|| {
                anyhow::anyhow!("Página hospedada indisponível ou limite de leitura atingido")
            })
        } else {
            read_page(settings, graph_path, n)
        };
        match body {
            Ok(body) if !body.trim().is_empty() => {
                let section = format!(
                    "## {}\n*{} · {} · {}*\n\n{}\n\n---\n\n",
                    n.display_title(),
                    crate::locale::type_label(&n.kind),
                    n.source,
                    n.slug,
                    body
                );
                if out.text.chars().count() + section.chars().count() > budget {
                    out.capped = true;
                    let remaining = budget.saturating_sub(out.text.chars().count());
                    if remaining > 200 {
                        out.text
                            .extend(section.chars().take(remaining.saturating_sub(40)));
                        out.text.push_str("\n\n[Cortado no limite escolhido]\n");
                        out.pages += 1;
                    }
                    break;
                }
                out.text.push_str(&section);
                out.pages += 1;
            }
            _ => out.missing += 1,
        }
    }
    out
}
pub fn obsidian_url(vault: &str, slug: &str) -> String {
    let mut url = url::Url::parse("obsidian://open").expect("valid static URL");
    url.query_pairs_mut()
        .append_pair("vault", vault)
        .append_pair("file", slug);
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mirror_reads_and_exports_never_escape_the_selected_root() {
        let d = tempfile::tempdir().unwrap();
        let base = d.path().join("source-default");
        std::fs::create_dir_all(base.join("people")).unwrap();
        std::fs::write(
            base.join("people/ada.md"),
            "---\r\ntitle: Ada\r\n---\r\n# Ada\nHuman identity",
        )
        .unwrap();
        std::fs::write(d.path().join("outside.md"), "secret").unwrap();
        assert!(page_path(&base, "../outside").is_err());
        assert!(page_path(&base, "C:/outside").is_err());
        assert!(page_path(&base, "..\\outside").is_err());
        let g =
            Graph::parse(br#"{"nodes":[{"id":1,"slug":"people/ada","type":"person"}],"links":[]}"#)
                .unwrap();
        let mut settings = Settings::default();
        settings.mirrors.insert("default".into(), base);
        let b = build(
            &g,
            &settings,
            None,
            Scope::Identity,
            None,
            &Default::default(),
            1000,
        );
        assert_eq!(b.pages, 1);
        assert!(b.text.contains("Human identity"));
        assert!(!b.text.contains("title: Ada"));
        assert!(obsidian_url("My & Vault", "notes/a?b").contains("%26"));
    }
    #[test]
    fn oversized_unicode_pages_produce_a_bounded_usable_export() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("identity.md"),
            "Identidade humana 🧠\n".repeat(1000),
        )
        .unwrap();
        let mut s = Settings::default();
        s.mirrors.insert("default".into(), d.path().to_owned());
        let g =
            Graph::parse(br#"{"nodes":[{"id":1,"slug":"identity","type":"person"}],"links":[]}"#)
                .unwrap();
        let b = build(
            &g,
            &s,
            None,
            Scope::Identity,
            None,
            &Default::default(),
            100,
        );
        assert!(b.capped);
        assert_eq!(b.pages, 1);
        assert!(b.text.contains("Identidade"));
        assert!(b.tokens() <= 100);
    }
}

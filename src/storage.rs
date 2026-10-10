use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub live: bool,
    pub ssh_host: String,
    pub poll_seconds: u64,
    pub graph_path: Option<PathBuf>,
    pub mirrors: BTreeMap<String, PathBuf>,
    pub vault: String,
    pub self_slug: String,
    pub reduce_motion: bool,
    pub show_labels: bool,
    pub appearance: crate::appearance::Appearance,
    pub hide_unlinked: bool,
    pub minimize_to_tray: bool,
    pub stars: BTreeMap<String, BTreeSet<String>>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            live: false,
            ssh_host: "gbrain-aws".into(),
            poll_seconds: 15,
            graph_path: None,
            mirrors: BTreeMap::new(),
            vault: "source-default".into(),
            self_slug: String::new(),
            reduce_motion: false,
            show_labels: true,
            appearance: crate::appearance::Appearance::default(),
            hide_unlinked: false,
            minimize_to_tray: false,
            stars: BTreeMap::new(),
        }
    }
}
impl Settings {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut settings: Self = serde_json::from_slice(&fs::read(path)?)
            .context("Não foi possível ler as configurações; o arquivo original foi preservado")?;
        settings.appearance.normalize();
        Ok(settings)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        atomic_write(path, &serde_json::to_vec_pretty(self)?)
    }
}
pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}
pub fn state_dir() -> PathBuf {
    std::env::var_os("GLIA_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::data_local_dir().unwrap_or_else(home).join("Glia"))
}
pub fn default_graph() -> PathBuf {
    home().join(".gbrain/viz/graph.json")
}
pub fn psyche_path() -> PathBuf {
    std::env::var_os("GLIA_PSYCHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".glia/psyche.md"))
}
pub fn resolve_graph(path: &Path) -> PathBuf {
    if path.is_dir() {
        if path.join("viz/graph.json").exists() {
            path.join("viz/graph.json")
        } else {
            path.join("graph.json")
        }
    } else {
        path.to_path_buf()
    }
}
pub fn brain_home(graph: &Path) -> Option<PathBuf> {
    let parent = graph.parent()?;
    Some(if parent.file_name().is_some_and(|n| n == "viz") {
        parent.parent()?.to_path_buf()
    } else {
        parent.to_path_buf()
    })
}
/// tempfile::persist uses the OS replace operation; never delete the destination first.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(data)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("Não foi possível salvar {}", path.display()))?;
    Ok(())
}
pub fn stable_hash(text: &str) -> u64 {
    text.as_bytes().iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
pub fn namespace(path: Option<&Path>) -> String {
    path.map(|p| {
        fs::canonicalize(p)
            .unwrap_or_else(|_| p.to_path_buf())
            .to_string_lossy()
            .to_lowercase()
    })
    .unwrap_or_else(|| "demo".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appearance_migrates_old_settings_and_round_trips_without_losing_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"live":true,"ssh_host":"my-aws","minimize_to_tray":true,"stars":{"aws:my-aws":["default:notes/a"]}}"#).unwrap();
        let mut settings = Settings::load(&path).unwrap();
        assert_eq!(
            settings.appearance.style,
            crate::appearance::GraphStyle::Constellation
        );
        assert!(settings.live && settings.minimize_to_tray);
        let stars = settings.stars.clone();
        settings.appearance.style = crate::appearance::GraphStyle::Gephi;
        settings.appearance.edge_opacity = 0.;
        settings.appearance.node_size = 2.;
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(
            loaded.appearance.style,
            crate::appearance::GraphStyle::Gephi
        );
        assert_eq!(loaded.appearance.edge_opacity, 0.);
        assert_eq!(loaded.appearance.node_size, 2.);
        assert_eq!(loaded.stars, stars);
        assert_eq!(loaded.ssh_host, "my-aws");
        fs::write(&path, r#"{"appearance":{"node_size":-50,"edge_opacity":3,"edge_width":-10,"label_size":1,"label_limit":1000000}}"#).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.appearance.node_size, 0.5);
        assert_eq!(loaded.appearance.edge_opacity, 1.);
        assert_eq!(loaded.appearance.edge_width, 0.2);
        assert_eq!(loaded.appearance.label_size, 8.);
        assert_eq!(loaded.appearance.label_limit, 150);
    }
    #[test]
    fn replaces_existing_files_and_keeps_brain_collections_separate() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        let mut s = Settings::default();
        s.stars
            .entry("a".into())
            .or_default()
            .insert("default:person/ada".into());
        s.save(&p).unwrap();
        s.vault = "updated".into();
        s.save(&p).unwrap();
        let loaded = Settings::load(&p).unwrap();
        assert_eq!(loaded.vault, "updated");
        assert!(!loaded.stars.contains_key("b"));
        fs::write(&p, "invalid").unwrap();
        assert!(Settings::load(&p).is_err());
        assert_eq!(fs::read_to_string(p).unwrap(), "invalid");
    }
}

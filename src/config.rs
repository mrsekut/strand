use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub enrich: EnrichConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EnrichConfig {
    pub skill: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        let path = crate::core::Core::repo_dir().join(".strand/config.toml");

        if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            Config::default()
        }
    }
}

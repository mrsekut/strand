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
        let path = dirs::config_dir().map(|d| d.join("strand/config.toml"));

        match path {
            Some(p) if p.exists() => {
                let content = std::fs::read_to_string(&p).unwrap_or_default();
                toml::from_str(&content).unwrap_or_default()
            }
            _ => Config::default(),
        }
    }
}

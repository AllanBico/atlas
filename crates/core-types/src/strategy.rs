use serde::Deserialize;
use toml::Value;

#[derive(Deserialize, Debug, Clone)]
pub struct StrategyConfig {
    pub name: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
    // This will hold the `params = { ... }` table from the TOML
    pub params: Value,
}

fn default_weight() -> f64 {
    1.0
}

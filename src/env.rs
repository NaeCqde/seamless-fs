use dotenvy::dotenv_override;
use serde::Deserialize;

#[derive(Clone, Default, Deserialize, Debug)]
pub struct Env {
    // General
    pub host: String,
    pub port: u16,

    // seamless file server
    pub origin: String,
    pub relay_url: String,
    pub token: String,
}

pub fn load_env() -> Result<Env, envy::Error> {
    // use dotenvy :)
    dotenv_override().ok();

    envy::from_env::<Env>()
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Kube Api Error: {0}")]
    KubeError(#[source] kube::Error),

    #[error("MissingObjectKey: {name:?}")]
    MissingObjectKey { name: &'static str },

    #[error("Cron Parse Error: {0}")]
    CronParseError(#[source] cron::error::Error),

    #[error("Schedule Error: {0}")]
    ScheduleError(String),

    #[error("Parse json Error: {0}")]
    JsonParseError(#[source] serde_json::Error),
}

impl From<kube::Error> for Error {
    fn from(error: kube::Error) -> Self {
        Error::KubeError(error)
    }
}

impl From<cron::error::Error> for Error {
    fn from(error: cron::error::Error) -> Self {
        Error::CronParseError(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::JsonParseError(error)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub mod manager;
pub use manager::Manager;

pub use manager::KrustJob;

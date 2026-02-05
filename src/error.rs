use thiserror::Error;

#[derive(Error, Debug)]
pub enum GritError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<octocrab::Error> for GritError {
    fn from(err: octocrab::Error) -> Self {
        GritError::Api(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, GritError>;

use crate::node::Node;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum FabrError {
    BadRequest(String),
    NotFound(String),
    Conflict { message: String, node: Option<Node> },
    Unprocessable(String),
    Io(String),
    Remote(String),
    Timeout(String),
}

impl FabrError {
    pub fn code(&self) -> u16 {
        match self {
            FabrError::BadRequest(_) => 400,
            FabrError::NotFound(_) => 404,
            FabrError::Conflict { .. } => 409,
            FabrError::Unprocessable(_) => 422,
            FabrError::Io(_) => 500,
            FabrError::Remote(_) => 502,
            FabrError::Timeout(_) => 504,
        }
    }
    pub fn message(&self) -> String {
        match self {
            FabrError::BadRequest(m)
            | FabrError::NotFound(m)
            | FabrError::Unprocessable(m)
            | FabrError::Io(m)
            | FabrError::Remote(m)
            | FabrError::Timeout(m) => m.clone(),
            FabrError::Conflict { message, .. } => message.clone(),
        }
    }
}

impl std::fmt::Display for FabrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.code(), self.message())
    }
}
impl std::error::Error for FabrError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDto {
    pub code: u16,
    pub message: String,
    pub node: Option<Node>,
}

impl From<FabrError> for ErrorDto {
    fn from(e: FabrError) -> Self {
        let code = e.code();
        let message = e.message();
        let node = match e {
            FabrError::Conflict { node, .. } => node,
            _ => None,
        };
        ErrorDto { code, message, node }
    }
}

pub fn client_error(op: &str, e: FabrError) -> ErrorDto {
    log::warn!("{op}: {} {}", e.code(), e.message());
    ErrorDto::from(e)
}

impl From<std::io::Error> for FabrError {
    fn from(e: std::io::Error) -> Self {
        FabrError::Io(e.to_string())
    }
}

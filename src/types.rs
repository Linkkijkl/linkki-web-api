use serde::Serialize;
use warp::reject;

/// Error type, which can partially get sent to user
#[derive(Debug, Default, Serialize)]
pub struct Error {
    /// The bit that is shown to user
    pub message: String,
    /// The bit that gets printed to logs, but not to user
    pub details: Option<String>,
}

impl reject::Reject for Error {}

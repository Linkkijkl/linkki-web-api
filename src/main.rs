use serde::Serialize;
use std::convert::Infallible;
use warp::Filter;
use warp::http::StatusCode;
use warp::{Rejection, Reply};

use crate::types::Error;

mod events;
pub mod types;

/// An API error serializable to JSON.
#[derive(Serialize)]
pub struct ErrorMessage {
    code: u16,
    message: String,
}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "404 - Not found";
    } else if let Some(error) = err.find::<Error>() {
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&error).unwrap_or_default()
        );
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = &error.message;
    } else {
        eprintln!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "500 - Internal server error";
    }
    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message: message.into(),
    });

    Ok(warp::reply::with_status(json, code))
}

#[tokio::main]
async fn main() {
    let routes = warp::any()
        .and(events::filter())
        .or(warp::path::end().map(|| "Hello world!"))
        .map(|reply| {
            warp::reply::with_header(reply, "Access-Control-Allow-Origin", "*")
        })
        .recover(handle_rejection);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

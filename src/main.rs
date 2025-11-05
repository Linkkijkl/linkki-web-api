use warp::Filter;

use std::convert::Infallible;
use warp::http::{Response, StatusCode};
use warp::{Rejection, Reply};

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let text;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        text = "404 - Not found";
    } else {
        code = StatusCode::INTERNAL_SERVER_ERROR;
        text = "500 - Internal server error";
        eprintln!("unhandled rejection: {:?}", err);
    }

    let response = Response::builder()
        .header("Content-Type", "text/html: charset=UTF-8")
        .body(text);

    Ok(warp::reply::with_status(response, code))
}

#[tokio::main]
async fn main() {
    let routes = warp::any()
        .and(warp::path::end().map(|| "Hello world!"))
        .recover(handle_rejection);

    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

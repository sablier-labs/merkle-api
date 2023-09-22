use futures::stream::{StreamExt, TryStreamExt};
use sea_orm::DbConn;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{http::Method, multipart::FormData, Filter, Rejection};

use crate::data_objects::query_param::Pagination;

mod entities;
mod repository;
mod services;
mod utils;
mod data_objects;

type WebResult<T> = std::result::Result<T, Rejection>;

#[tokio::main]
async fn main() {
    let db_pool = services::db::establish_connection()
        .await
        .expect("Failed to create db pool");

    let cors = warp::cors()
        .allow_methods(&[Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_origins(vec!["http://localhost:3000/", "http://localhost:8000/"])
        .allow_headers(vec!["content-type"])
        .allow_credentials(true);

    let health_checker = warp::path!("api" / "healthchecker")
        .and(warp::get())
        .and_then(services::handler::health_checker_handler);

    let upload_route = warp::path!("api" / "upload")
        .and(warp::post())
        .and(warp::multipart::form().max_length(100_000_000))
        .and(with_db(db_pool.clone()))
        .and_then(services::handler::upload_handler);

    let get_recipients_route = warp::path!("api" / "entries" / String)
        .and(warp::get())
        .and(warp::query::query::<Pagination>())
        .and(with_db(db_pool.clone()))
        .and_then(services::handler::get_recipients_handler);

    let get_campaign_route = warp::path!("api" / "campaigns" / String)
        .and(warp::get())
        .and(with_db(db_pool.clone()))
        .and_then(services::handler::get_campaign_handler);

    let publish_route = warp::path!("api" / "publish" / String)
        .and(warp::post())
        .and(with_db(db_pool.clone()))
        .and_then(services::handler::publish_campaign_handler);

    let routes = health_checker
        .with(cors)
        .with(warp::log("api"))
        .or(upload_route)
        .or(get_recipients_route)
        .or(get_campaign_route)
        .or(publish_route);

    println!("🚀 Server started successfully");
    warp::serve(routes).run(([0, 0, 0, 0], 8000)).await;
}

fn with_db(
    db_pool: Arc<Mutex<DbConn>>,
) -> impl Filter<Extract = (Arc<Mutex<DbConn>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db_pool.clone())
}

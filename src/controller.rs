use warp::{http::Method, Filter};

pub mod create;
pub mod create_solana;
pub mod eligibility;
pub mod eligibility_solana;
pub mod health;
pub mod validity;

/// Handle the rejection raised by the Warp framework.
async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, std::convert::Infallible> {
    Ok(warp::reply::json(&format!("{err:?}")))
}

/// Binds all the routes into a single API and create a proper configuration with the allowed headers and CORS
/// configuration.
pub fn build_routes() -> impl warp::Filter<Extract = impl warp::Reply> + Clone {
    let cors = warp::cors()
        .allow_methods(&[Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_any_origin()
        .allow_headers(vec!["Origin", "Content-Type", "X-Auth-Token", "X-AppId"]);

    let health = health::build_route();
    let create = create::build_route();
    let create_solana = create_solana::build_route();
    let eligibility = eligibility::build_route();
    let eligibility_solana = eligibility_solana::build_route();
    let validity = validity::build_route();

    health
        .or(eligibility)
        .or(eligibility_solana)
        .or(create)
        .or(create_solana)
        .or(validity)
        .recover(handle_rejection)
        .with(cors)
        .with(warp::log("api"))
}

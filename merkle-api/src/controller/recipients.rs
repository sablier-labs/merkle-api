use crate::{
    data_objects::dto::{RecipientDto, RecipientPageDto},
    data_objects::query_param::Pagination,
    data_objects::response::{BadRequestResponse, RecipientsSuccessResponse, self},
    database::management::with_db,
    repository, WebResult,
};

use sea_orm::DbConn;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{reply::json, Filter, Reply};

async fn get_recipients_handler(
    guid: String,
    pagination: Pagination,
    db: Arc<Mutex<DbConn>>,
) -> WebResult<impl Reply> {
    let db = db.lock().await;
    let db_conn = db.clone();

    let recipients = repository::recipient::get_recipients_by_campaign_guid(
        guid,
        pagination.page_number,
        pagination.page_size,
        &db_conn,
    )
    .await;

    if let Err(_) = recipients {
        let response_json = &BadRequestResponse {
            message: "There was a problem processing your request.".to_string(),
        };
        return Ok(response::internal_server_error(json(response_json)));
    }
    let recipients = recipients.unwrap();
    let response_json = &RecipientsSuccessResponse {
        status: "Request successful".to_string(),
        page: RecipientPageDto {
            page_number: pagination.page_number,
            page_size: pagination.page_size,
            recipients: recipients
                .into_iter()
                .map(|x| RecipientDto {
                    address: x.address,
                    amount: x.amount.parse().unwrap(),
                })
                .collect(),
        },
    };
    return Ok(response::ok(json(response_json)));
}

pub fn build_route(
    db: Arc<Mutex<DbConn>>,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "entries" / String)
        .and(warp::get())
        .and(warp::query::query::<Pagination>())
        .and(with_db(db))
        .and_then(get_recipients_handler)
}

use utoipa::OpenApi;
use warp::Filter;

use crate::{
    controller::{create, create_solana, eligibility, eligibility_solana, health, validity},
    data_objects::{
        dto::{PersistentCampaignDto, RecipientDto},
        query_param::{Create, Eligibility, Validity},
        response::{
            EligibilityResponse, GeneralErrorResponse, UploadSuccessResponse, ValidResponse,
            ValidationErrorResponse,
        },
    },
    utils::csv_validator::ValidationError,
};

/// OpenAPI documentation structure
#[derive(OpenApi)]
#[openapi(
    paths(
        create::handler_to_warp,
        create_solana::handler_to_warp,
        eligibility::handler_to_warp,
        eligibility_solana::handler_to_warp,
        validity::handler_to_warp,
        health::handler_to_warp,
    ),
    components(
        schemas(
            // Request query parameters
            Create,
            Eligibility,
            Validity,
            // Response schemas
            UploadSuccessResponse,
            EligibilityResponse,
            ValidResponse,
            GeneralErrorResponse,
            ValidationErrorResponse,
            ValidationError,
            // DTOs
            PersistentCampaignDto,
            RecipientDto,
            // Health response
            health::HealthResponse,
        )
    ),
    tags(
        (name = "Campaign", description = "Endpoints for creating Merkle airdrop campaigns"),
        (name = "Verification", description = "Endpoints for verifying campaign eligibility and validity"),
        (name = "Health", description = "Health check endpoint")
    ),
    info(
        title = "Sablier Merkle API",
        version = "0.0.1",
        description = "A web API for generating and verifying Merkle trees used in Sablier V2",
        contact(
            name = "Sablier Labs Ltd",
            email = "contact@sablier.com",
            url = "https://github.com/sablier-labs/v2-merkle-api"
        )
    )
)]
pub struct ApiDoc;

/// Handler to serve the OpenAPI spec as JSON
async fn openapi_spec() -> impl warp::Reply {
    warp::reply::json(&ApiDoc::openapi())
}

/// Build the route for serving the OpenAPI specification
pub fn build_openapi_route() -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "openapi.json").and(warp::get()).and_then(|| async { Ok::<_, warp::Rejection>(openapi_spec().await) })
}

/// Handler to serve Swagger UI HTML
async fn swagger_ui_handler() -> impl warp::Reply {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Sablier Merkle API - Swagger UI</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-standalone-preset.js"></script>
    <script>
        window.onload = function() {
            window.ui = SwaggerUIBundle({
                url: '/api/openapi.json',
                dom_id: '#swagger-ui',
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                layout: "StandaloneLayout"
            });
        };
    </script>
</body>
</html>"#.to_string();
    warp::reply::html(html)
}

/// Build the route for serving Swagger UI
pub fn build_swagger_ui_route() -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("swagger-ui")
        .and(warp::get())
        .and_then(|| async { Ok::<_, warp::Rejection>(swagger_ui_handler().await) })
}

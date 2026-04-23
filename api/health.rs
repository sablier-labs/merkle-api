use sablier_merkle_api::controller::health;
use vercel_runtime as Vercel;

#[tokio::main]
async fn main() -> Result<(), Vercel::Error> {
    Vercel::run(Vercel::service_fn(handler)).await
}

pub async fn handler(_req: Vercel::Request) -> Result<Vercel::Response<Vercel::ResponseBody>, Vercel::Error> {
    health::handler_to_vercel().await
}

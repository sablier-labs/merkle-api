use sablier_merkle_api::controller::create_solana;
use vercel_runtime as Vercel;

#[tokio::main]
async fn main() -> Result<(), Vercel::Error> {
    Vercel::run(Vercel::service_fn(handler)).await
}

pub async fn handler(req: Vercel::Request) -> Result<Vercel::Response<Vercel::ResponseBody>, Vercel::Error> {
    create_solana::handler_to_vercel(req).await
}

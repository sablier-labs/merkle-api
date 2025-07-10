use sablier_merkle_api::controller::create_solana;
use vercel_runtime as Vercel;

#[tokio::main]
async fn main() -> Result<(), Vercel::Error> {
    Vercel::run(handler).await
}

pub async fn handler(req: Vercel::Request) -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    create_solana::handler_to_vercel(req).await
}

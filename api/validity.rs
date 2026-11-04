use sablier_merkle_api::{controller::validity, utils::secrets};
use vercel_runtime as Vercel;

fn main() -> Result<(), Vercel::Error> {
    // Decrypt and export secrets before any runtime thread exists, so `set_var` cannot race another `getenv`.
    secrets::load()?;
    tokio::runtime::Runtime::new()?.block_on(Vercel::run(Vercel::service_fn(handler)))
}

pub async fn handler(req: Vercel::Request) -> Result<Vercel::Response<Vercel::ResponseBody>, Vercel::Error> {
    validity::handler_to_vercel(req).await
}

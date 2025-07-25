#[cfg(test)]
pub mod async_test {
    use mockito::Server;
    use once_cell::sync::Lazy;
    use tokio::sync::{Mutex, MutexGuard};

    pub static SERVER: Lazy<Mutex<Server>> = Lazy::new(|| {
        Mutex::new(mockito::Server::new_with_opts(mockito::ServerOpts { port: 8000, ..Default::default() }))
    });
    pub fn setup_env_vars(server: &MutexGuard<'_, Server>) {
        let host = server.host_with_port();
        let parts: Vec<&str> = host.split(':').collect();
        let port = parts[1];
        let server_host = format!("http://localhost:{}", port);

        std::env::set_var("PINATA_ACCESS_TOKEN", "mock_pinata_access_token");
        std::env::set_var("IPFS_GATEWAY", server_host.clone());
        std::env::set_var("PINATA_API_KEY", "mock_pinata_api_key");
        std::env::set_var("PINATA_SECRET_API_KEY", "mock_pinata_secret_key");
        std::env::set_var("PINATA_API_SERVER", server_host);
    }
}
pub mod csv_validator;
pub mod solana_merkle;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = server::AppConfig::load()?;
    let app = if let Some(database_url) = server::database_url_or_in_memory_from_env(&config)? {
        let repo = storage::PostgresRepositories::connect(&database_url).await?;
        storage::MIGRATOR.run(repo.pool()).await?;
        server::router_with_auth_store(config.clone(), std::sync::Arc::new(repo))
    } else {
        server::router(config.clone())
    };
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = server::AppConfig::load()?;
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    axum::serve(listener, server::router(config)).await?;
    Ok(())
}

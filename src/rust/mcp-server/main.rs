#[tokio::main]
async fn main() -> anyhow::Result<()> {
    mcp_server::run_stdio_server().await
}

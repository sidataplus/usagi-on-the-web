#[tokio::main]
async fn main() -> anyhow::Result<()> {
    api_worker::run_worker().await
}

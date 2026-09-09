#[tokio::main]
async fn main() -> anyhow::Result<()> {
    zhiqiaoapi::run().await
}

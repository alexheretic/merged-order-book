#[tokio::main]
async fn main() -> anyhow::Result<()> {
    merged_order_book_grpc::start().await
}

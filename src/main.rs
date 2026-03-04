#[tokio::main]
async fn main() {
    if let Err(err) = castkit::run().await {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

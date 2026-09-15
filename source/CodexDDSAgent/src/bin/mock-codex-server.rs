#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = vha_codex_dds_agent::mock_server::run().await {
        eprintln!("mock-codex-server error: {error}");
        std::process::exit(1);
    }
}

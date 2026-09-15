use clap::Parser;

fn main() {
    let args = vha_codex_dds_agent::cli::Cli::parse();
    if let Err(error) = vha_codex_dds_agent::cli::run(args) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

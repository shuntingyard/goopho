use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

// TODO When using ::* it might bee nicer to move make_* functions into their
// own submodule, allowing to `use goopho::calculations::make::*` (or even as prelude)?
use goopho::{calculations::*, persistence, walk_and_calculate};

#[derive(clap::ValueEnum, Clone, Debug)]
enum CalcFnType {
    Dhash,
    Thumbnails,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
/// App blah...
struct Cli {
    /// Input directory
    #[arg(short, long, value_name = "DIR")]
    imagedir: PathBuf,

    /// Function to run on input directory
    #[clap(value_enum, default_value_t=CalcFnType::Dhash)]
    function: CalcFnType,

    /// Do a full async read when set
    #[arg(short, long)]
    async_full: bool,
}

#[tokio::main]
async fn main() {
    // Subscribe to traces.
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env()) // Read trace levels from RUST_LOG env var.
        .init();

    // Parse command line arguments.
    let cli = Cli::parse();
    tracing::trace!("{cli:#?}");

    // Set up a persistence store.
    //let store = persistence::StdoutStore;
    let store = persistence::SqliteStore::build().await.unwrap();

    // Define the things to calculate.
    let mut calcs: Vec<CalcFn> = vec![];
    match cli.function {
            CalcFnType::Dhash => calcs.push(make_dhash as CalcFn),
            CalcFnType::Thumbnails => calcs.push(make_thumbnail as CalcFn),
    };

    walk_and_calculate(cli.imagedir, store, calcs, cli.async_full)
        .await
        .unwrap();
}

use argh::FromArgs;

use goopho::fsvisitor::{visit, visit_sync};

#[derive(FromArgs)]
/// Traverse directories for goopho
struct CmdlArgs {
    /// where to start
    #[argh(positional)]
    path: String,

    /// async traversal
    #[argh(switch, short = 'a')]
    go_async: bool,
}

#[tokio::main]
async fn main() {
    let args: CmdlArgs = argh::from_env();

    if args.go_async {
        visit(&args.path).await;
    } else {
        visit_sync(&args.path);
    }
}

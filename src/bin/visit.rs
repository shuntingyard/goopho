use goopho::fsvisitor::visit;

#[tokio::main]
async fn main() {
    let path = std::env::args().nth(1).expect("Arg1 must be filepath");
    visit(&path).await;
}

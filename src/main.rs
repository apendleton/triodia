use std::env;
use std::sync::Arc;

mod index;
mod util;
mod server;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let filename = args.get(1).expect("must provide one filename");

    let idx = Arc::new(index::load(&filename).unwrap());

    println!("starting server on port 9005...");
    server::start(idx, 9005).await
}

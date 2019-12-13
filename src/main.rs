use std::error;

use log::debug;
use structopt::StructOpt;

use cached_path::cached_path;

#[derive(Debug, StructOpt)]
#[structopt(name = "cached_path", about = "get the cached path to a resource")]
struct Opt {
    /// The resource path.
    #[structopt()]
    resource: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    env_logger::init();
    let opt = Opt::from_args();
    debug!("{:?}", opt);

    let resource = opt.resource;
    let path = cached_path(&resource[..]).await?;
    println!("{}", path.to_string_lossy());
    Ok(())
}

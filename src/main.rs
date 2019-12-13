use structopt::StructOpt;

use cached_path::cached_path;
use log::debug;

#[derive(Debug, StructOpt)]
#[structopt(name = "cached_path", about = "get the cached path to a resource")]
struct Opt {
    /// The resource path.
    #[structopt()]
    resource: String,
}

fn main() {
    env_logger::init();
    let opt = Opt::from_args();
    debug!("{:?}", opt);

    let resource = opt.resource;
    let path = cached_path(&resource[..]).unwrap();
    println!("{}", path.to_string_lossy());
}

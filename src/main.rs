use structopt::StructOpt;

use cached_path::cached_path;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cached_path",
    about = "get the cached path to a resource",
)]
struct Opt {
    #[structopt()]
    /// The resource path.
    resource: String,
}

fn main() {
    let opt = Opt::from_args();
    let resource = opt.resource;
    let path = cached_path(&resource[..]).unwrap();
    println!("{}", path.to_string_lossy());
}

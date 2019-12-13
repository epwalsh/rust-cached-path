use std::error;
use std::process;

use log::debug;
use structopt::StructOpt;
use tokio::sync::mpsc::channel;

use cached_path::cached_path;

#[derive(Debug, StructOpt)]
#[structopt(name = "cached_path", about = "get the cached path to a resource")]
struct Opt {
    /// The resource path.
    #[structopt()]
    resource: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    env_logger::init();
    let opt = Opt::from_args();
    debug!("{:?}", opt);

    let (tx, mut rx) = channel(100);

    let mut retval = 0;
    for resource in opt.resource {
        let mut tx = tx.clone();
        tokio::spawn(async move {
            let result = cached_path(&resource[..]).await.map_err(|_| {"failed"});
            if tx.send((resource, result)).await.is_err() {
                std::process::exit(1);
            };
        });
    }

    drop(tx);

    while let Some((resource, result)) = rx.recv().await {
        if let Ok(path) = result {
            println!("{} -> {}", resource, path.to_string_lossy());
        } else {
            retval = 1;
        }
    }

    if retval > 0 {
        process::exit(retval);
    }

    Ok(())
}

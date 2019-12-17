use std::collections::HashMap;
use std::error;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use log::debug;
use reqwest::Client;
use structopt::StructOpt;
use tokio::sync::mpsc::channel;

use cached_path::Cache;

#[derive(Debug, StructOpt)]
#[structopt(name = "cached_path", about = "Get the cached path to a resource.")]
struct Opt {
    #[structopt()]
    /// The resource paths.
    resource: Vec<String>,

    #[structopt(long = "root", env = "RUST_CACHED_PATH_ROOT")]
    /// The root cache directory. Defaults to a subdirectory 'cache' of the default
    /// system temporary directory.
    root: Option<PathBuf>,

    #[structopt(long = "timeout")]
    /// Set a request timeout.
    timeout: Option<u64>,

    #[structopt(long = "connect-timeout")]
    /// Set a timeout for the connect phase of the HTTP client.
    connect_timeout: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    env_logger::init();
    let opt = Opt::from_args();

    debug!("{:?}", opt);

    let http_client = build_http_client(&opt)?;
    let root = opt
        .root
        .unwrap_or_else(|| std::env::temp_dir().join("cache/"));
    let cache = Cache::new(root, http_client).await?;

    let (tx, mut rx) = channel(100);

    let mut retval = 0;
    for resource in &opt.resource {
        let mut tx = tx.clone();
        let resource = resource.clone();
        let cache = cache.clone();
        tokio::spawn(async move {
            let result = cache.cached_path(&resource[..]).await.map_err(|_| "failed");
            if tx.send((resource, result)).await.is_err() {
                std::process::exit(1);
            };
        });
    }

    drop(tx);

    let mut cached_paths: HashMap<String, PathBuf> = HashMap::new();

    while let Some((resource, result)) = rx.recv().await {
        if let Ok(path) = result {
            cached_paths.insert(resource, path);
        } else {
            retval = 1;
        }
    }

    if retval > 0 {
        process::exit(retval);
    }

    for resource in &opt.resource {
        let path = cached_paths.get(&resource[..]).unwrap();
        println!("{}", path.to_string_lossy());
    }

    Ok(())
}

fn build_http_client(opt: &Opt) -> Result<Client, Box<dyn error::Error>> {
    let mut http_client_builder = Client::builder();
    if let Some(timeout) = opt.timeout {
        http_client_builder = http_client_builder.timeout(Duration::from_secs(timeout));
    }
    if let Some(connect_timeout) = opt.connect_timeout {
        http_client_builder =
            http_client_builder.connect_timeout(Duration::from_secs(connect_timeout));
    }
    Ok(http_client_builder.build()?)
}

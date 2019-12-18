use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use exitfailure::ExitFailure;
use log::debug;
use structopt::StructOpt;
use tokio::sync::mpsc::channel;

use cached_path::Cache;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cached_path",
    about = "Get the cached path to a resource.",
    setting = structopt::clap::AppSettings::ColoredHelp,
)]
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
async fn main() -> Result<(), ExitFailure> {
    env_logger::init();
    let opt = Opt::from_args();

    debug!("{:?}", opt);

    let mut cache_builder = Cache::builder();
    if let Some(root) = opt.root {
        cache_builder = cache_builder.root(root);
    }
    if let Some(timeout) = opt.timeout {
        cache_builder = cache_builder.timeout(Duration::from_secs(timeout));
    }
    if let Some(connect_timeout) = opt.connect_timeout {
        cache_builder = cache_builder.connect_timeout(Duration::from_secs(connect_timeout));
    }

    let cache = cache_builder.build().await?;

    let (tx, mut rx) = channel(100);

    for resource in &opt.resource {
        let mut tx = tx.clone();
        let resource = resource.clone();
        let cache = cache.clone();
        tokio::spawn(async move {
            let result = cache.cached_path(&resource[..]).await;
            if tx.send((resource, result)).await.is_err() {
                std::process::exit(1);
            };
        });
    }

    drop(tx);

    let mut cached_paths: HashMap<String, PathBuf> = HashMap::new();

    while let Some((resource, result)) = rx.recv().await {
        let path = result?;
        cached_paths.insert(resource, path);
    }

    for resource in &opt.resource {
        let path = cached_paths.get(&resource[..]).unwrap();
        println!("{}", path.to_string_lossy());
    }

    Ok(())
}

use anyhow::Result;
use log::debug;
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;

use cached_path::{Cache, Error};

#[derive(Debug, StructOpt)]
#[structopt(
    name = "cached-path",
    about = "Get the cached path to a resource.",
    setting = structopt::clap::AppSettings::ColoredHelp,
)]
struct Opt {
    #[structopt()]
    /// The resource path.
    resource: String,

    #[structopt(long = "dir", env = "RUST_CACHED_PATH_ROOT")]
    /// The cache directory. Defaults to a subdirectory named 'cache' of the default
    /// system temporary directory.
    dir: Option<PathBuf>,

    #[structopt(long = "subdir")]
    /// The subdirectory, relative to the cache root directory to use.
    subdir: Option<String>,

    #[structopt(long = "timeout")]
    /// Set a request timeout.
    timeout: Option<u64>,

    #[structopt(long = "connect-timeout")]
    /// Set a timeout for the connect phase of the HTTP client.
    connect_timeout: Option<u64>,

    #[structopt(long = "max-retries", default_value = "3")]
    /// Set the maximum number of times to retry an HTTP request. Retriable failures are tried
    /// again with exponential backoff.
    max_retries: u32,

    #[structopt(long = "max-backoff", default_value = "5000")]
    /// Set the maximum backoff delay in milliseconds for retrying HTTP requests.
    max_backoff: u32,

    #[structopt(long = "freshness-lifetime")]
    /// Set the a default freshness lifetime (in seconds) for cached resources.
    freshness_lifetime: Option<u64>,

    #[structopt(long = "offline")]
    /// Only use offline features.
    offline: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let opt = Opt::from_args();

    debug!("{:?}", opt);

    let cache = build_cache_from_opt(&opt)?;
    let path = cache.cached_path_in_subdir(&opt.resource, opt.subdir.as_deref())?;
    println!("{}", path.to_string_lossy());

    Ok(())
}

fn build_cache_from_opt(opt: &Opt) -> Result<Cache, Error> {
    let mut cache_builder = Cache::builder().offline(opt.offline);
    if let Some(dir) = &opt.dir {
        cache_builder = cache_builder.dir(dir.clone());
    }
    if let Some(timeout) = opt.timeout {
        cache_builder = cache_builder.timeout(Duration::from_secs(timeout));
    }
    if let Some(connect_timeout) = opt.connect_timeout {
        cache_builder = cache_builder.connect_timeout(Duration::from_secs(connect_timeout));
    }
    if let Some(freshness_lifetime) = opt.freshness_lifetime {
        cache_builder = cache_builder.freshness_lifetime(freshness_lifetime);
    }
    cache_builder = cache_builder.max_retries(opt.max_retries);
    cache_builder = cache_builder.max_backoff(opt.max_backoff);
    cache_builder.build()
}

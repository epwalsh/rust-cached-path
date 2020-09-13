use crate::Error;
use std::io::Read;
use std::time::Duration;

const ETAG: &str = "ETag";

/// A `Client` fetches remote resources for the `Cache`.
#[derive(Debug)]
pub(crate) struct Client {
    /// An optional timeout for downloading remote resources.
    timeout: Option<Duration>,
    /// An optional timeout for establishing a connection to remote resources.
    connect_timeout: Option<Duration>,
}

impl Client {
    pub(crate) fn new(timeout: Option<Duration>, connect_timeout: Option<Duration>) -> Self {
        Self {
            timeout,
            connect_timeout,
        }
    }

    fn check_response(response: &ureq::Response) -> Result<(), Error> {
        if response.error() {
            match response.synthetic_error() {
                Some(err) => Err(Error::from(err)),
                None => Err(Error::HttpStatusError(response.status())),
            }
        } else {
            Ok(())
        }
    }

    pub(crate) fn download_resource(&self, resource: &str) -> Result<impl Read, Error> {
        let mut request = ureq::get(resource);
        if let Some(timeout) = self.connect_timeout {
            request.timeout_connect(timeout.as_millis() as u64);
        }
        if let Some(timeout) = self.timeout {
            request.timeout(timeout);
        }
        let response = request.call();
        Self::check_response(&response)?;

        Ok(response.into_reader())
    }

    pub(crate) fn get_etag(&self, resource: &str) -> Result<Option<String>, Error> {
        let mut request = ureq::head(resource);
        if let Some(timeout) = self.connect_timeout {
            request.timeout_connect(timeout.as_millis() as u64);
        }
        if let Some(timeout) = self.timeout {
            request.timeout(timeout);
        }
        let response = request.call();
        Self::check_response(&response)?;
        Ok(response.header(ETAG).map(String::from))
    }
}

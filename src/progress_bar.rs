use std::io::{self, Write};
use std::time::Instant;

/// Progress bar types.
#[derive(Debug, Clone)]
pub enum ProgressBar {
    /// Progress bar with the highest verbosity.
    ///
    /// This should only be used if you're running `cached-path` from an interactive terminal.
    Full,
    /// Progress bar with that produces minimal output.
    ///
    /// This is a good option to use if your output is being captured to a file but you
    /// still want to see a progress bar for downloads.
    Light,
}

impl Default for ProgressBar {
    fn default() -> Self {
        ProgressBar::Light
    }
}

impl ProgressBar {
    pub(crate) fn get_full_progress_bar(
        resource: &str,
        content_length: Option<u64>,
    ) -> indicatif::ProgressBar {
        let progress_bar = match content_length {
            Some(length) => {
                let progress_bar = indicatif::ProgressBar::new(length);
                progress_bar.set_style(indicatif::ProgressStyle::default_spinner().template(
                    "{percent}% {wide_bar:.cyan/blue} {bytes_per_sec},<{eta} [{bytes}, {elapsed}]",
                ));
                progress_bar
            }
            None => {
                let progress_bar = indicatif::ProgressBar::new_spinner();
                progress_bar.set_style(
                    indicatif::ProgressStyle::default_spinner()
                        .template("{spinner} {bytes_per_sec} [{bytes}, {elapsed}] {msg}"),
                );
                progress_bar
            }
        };
        progress_bar.println(format!("Downloading {}", resource));
        progress_bar.set_draw_delta(1_000_000);
        progress_bar
    }

    pub(crate) fn get_light_download_wrapper<W: Write>(
        resource: &str,
        content_length: Option<u64>,
        writer: W,
    ) -> LightDownloadWrapper<W> {
        LightDownloadWrapper::new(resource, content_length, writer)
    }
}

#[derive(Debug)]
pub(crate) struct LightDownloadWrapper<W: Write> {
    start_time: Instant,
    bytes: usize,
    bytes_since_last_update: usize,
    writer: W,
}

impl<W: Write> LightDownloadWrapper<W> {
    pub(crate) fn new(resource: &str, content_length: Option<u64>, writer: W) -> Self {
        if let Some(size) = content_length {
            eprint!(
                "Downloading {} [{}]...",
                resource,
                indicatif::HumanBytes(size)
            );
        } else {
            eprint!("Downloading {}...", resource);
        }
        io::stderr().flush().ok();
        Self {
            start_time: Instant::now(),
            bytes: 0,
            bytes_since_last_update: 0,
            writer,
        }
    }

    pub(crate) fn finish(&self) {
        let duration = Instant::now().duration_since(self.start_time);
        eprint!(
            " ✓ Done! Finished in {}\n",
            indicatif::HumanDuration(duration)
        );
        io::stderr().flush().ok();
    }
}

impl<W: Write> Write for LightDownloadWrapper<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice]) -> io::Result<usize> {
        self.writer.write_vectored(bufs)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.writer.write_all(buf).map(|()| {
            let chunk_size = buf.len();
            self.bytes_since_last_update += chunk_size;
            // Update every 100 MBs.
            if self.bytes_since_last_update > 100_000_000 {
                eprint!(".");
                io::stderr().flush().ok();
                self.bytes_since_last_update = 0;
            }
            self.bytes += chunk_size;
        })
    }
}

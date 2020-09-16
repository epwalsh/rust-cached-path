use std::io::{self, Write};
use std::time::Instant;

/// Progress bar types.
///
/// This can be set with
/// [`CacheBuilder::progress_bar()`](struct.CacheBuilder.html#method.progress_bar).
#[derive(Debug, Clone)]
pub enum ProgressBar {
    /// Gives pretty, verbose progress bars.
    Full,
    /// Gives progress bars with minimal output.
    ///
    /// This is a good option to use if your output is being captured to a file but you
    /// still want to see progress updates.
    Light,
}

impl Default for ProgressBar {
    fn default() -> Self {
        ProgressBar::Full
    }
}

impl ProgressBar {
    pub(crate) fn get_download_wrapper(content_length: Option<u64>) -> DownloadWrapper {
        DownloadWrapper::new(content_length)
    }

    pub(crate) fn get_light_download_wrapper<W: Write>(
        resource: &str,
        content_length: Option<u64>,
        writer: W,
    ) -> LightDownloadWrapper<W> {
        LightDownloadWrapper::new(resource, content_length, writer)
    }
}

pub(crate) struct DownloadWrapper {
    bar: indicatif::ProgressBar,
}

impl DownloadWrapper {
    pub(crate) fn new(content_length: Option<u64>) -> Self {
        let bar = match content_length {
            Some(length) => {
                let bar = indicatif::ProgressBar::new(length);
                bar.set_style(
                    indicatif::ProgressStyle::default_bar()
                    .progress_chars("=>-")
                    .template(
                        "{msg:.bold.cyan/blue} [{bar:20.cyan/blue}][{percent}%] {bytes}/{total_bytes:.bold} |{bytes_per_sec}|",
                    )
                );
                bar
            }
            None => {
                let bar = indicatif::ProgressBar::new_spinner();
                bar.set_style(
                    indicatif::ProgressStyle::default_bar()
                        .tick_strings(&[
                            "⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖",
                            "⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐",
                            "⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒",
                            "⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋",
                            "⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈",
                            "⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈⠉",
                            "⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈⠉⠙⠚",
                            "⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈⠉⠙⠚⠒⠂",
                            "⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈⠉⠙⠚⠒⠂⠂⠒",
                            "⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈⠉⠙⠚⠒⠂⠂⠒⠲⠴",
                            "⠒⠐⠐⠒⠓⠋⠉⠈⠈⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄",
                            "⠐⠒⠓⠋⠉⠈⠈⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤",
                            "⠓⠋⠉⠈⠈⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠",
                        ])
                        .template(
                            "{msg:.bold.cyan/blue} [{spinner:.cyan/blue}] {bytes:.bold} |{bytes_per_sec}|",
                        ),
                );
                bar
            }
        };
        bar.set_message("Downloading");
        // Update every 1 MBs.
        // NOTE: If we don't set this, the updates happen WAY too frequently and it makes downloads
        // take about twice as long.
        bar.set_draw_delta(1_000_000);
        Self { bar }
    }

    pub(crate) fn wrap_write<W: Write>(&self, write: W) -> impl Write {
        self.bar.wrap_write(write)
    }

    pub(crate) fn finish(&self) {
        self.bar.set_message("Downloaded");
        self.bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{msg:.green.bold} {total_bytes:.bold} in {elapsed}"),
        );
        self.bar.finish_at_current_pos();
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

    pub(crate) fn tick(&mut self, chunk_size: usize) {
        self.bytes_since_last_update += chunk_size;
        // Update every 100 MBs.
        if self.bytes_since_last_update > 100_000_000 {
            eprint!(".");
            io::stderr().flush().ok();
            self.bytes_since_last_update = 0;
        }
        self.bytes += chunk_size;
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
            self.tick(buf.len());
        })
    }
}

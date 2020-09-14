use std::io;

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
    pub(crate) fn get_download_wrapper<W: io::Write>(
        &self,
        content_length: Option<u64>,
        writer: W,
    ) -> ProgressBarDownloadWrap<W> {
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
        let update_every = match self {
            ProgressBar::Full => {
                // update every 1 MB.
                1_000_000
            }
            ProgressBar::Light => {
                progress_bar.set_style(
                    indicatif::ProgressStyle::default_spinner()
                        .template("[{bytes}, {elapsed}] Downloading...{msg}"),
                );
                // update every 100 MB.
                100_000_000
            }
        };
        ProgressBarDownloadWrap::new(self.clone(), progress_bar, writer, update_every)
    }
}

pub(crate) struct ProgressBarDownloadWrap<W> {
    level: ProgressBar,
    bar: indicatif::ProgressBar,
    wrap: W,
    buffered_progress: usize,
    update_every: usize,
    updates: usize,
}

impl<W> ProgressBarDownloadWrap<W> {
    pub(crate) fn new(
        level: ProgressBar,
        bar: indicatif::ProgressBar,
        wrap: W,
        update_every: usize,
    ) -> Self {
        Self {
            level,
            bar,
            wrap,
            buffered_progress: 0,
            update_every,
            updates: 0,
        }
    }

    pub(crate) fn finalize(&self, bytes: u64) {
        self.bar.set_position(bytes);
        match self.level {
            ProgressBar::Light => {
                let msg = format!("{} ✓ Done!", ".".repeat(self.updates));
                self.bar.set_message(&msg);
            }
            _ => self.bar.set_message("✓ Done!"),
        };
        self.bar.finish_at_current_pos();
    }
}

impl<W: io::Write> io::Write for ProgressBarDownloadWrap<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.wrap.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.wrap.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice]) -> io::Result<usize> {
        self.wrap.write_vectored(bufs)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.wrap.write_all(buf).map(|()| {
            let inc = buf.len();
            self.buffered_progress += inc;
            if self.buffered_progress > self.update_every {
                self.updates += 1;
                self.bar.inc(self.buffered_progress as u64);
                if let ProgressBar::Light = self.level {
                    let msg = ".".repeat(self.updates);
                    self.bar.set_message(&msg);
                }
                self.buffered_progress = 0;
            }
        })
    }
}

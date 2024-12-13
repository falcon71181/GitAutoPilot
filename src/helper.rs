use notify::{Config, Event, RecommendedWatcher, Watcher, WatcherKind};
use std::sync::mpsc;
use std::time::Duration;

/// Creates a file system watcher with optimized configuration based on the recommended watcher type.
///
/// This function initializes a file system watcher that can detect changes in the file system.
/// It adapts the watcher configuration based on the detected watcher kind, providing
/// a custom polling interval for poll-based watchers.
///
/// # Parameters
/// - `tx`: A channel sender for broadcasting file system events or errors
///
/// # Returns
/// A boxed file system watcher implementing the `Watcher` trait
///
/// # Errors
/// Returns a `notify::Error` if the watcher fails to initialize
///
/// # Examples
/// ```
/// let (tx, rx) = mpsc::channel();
/// let watcher = create_watcher(tx)?;
///
pub fn create_watcher(
    tx: mpsc::Sender<Result<Event, notify::Error>>,
) -> Result<Box<dyn Watcher>, notify::Error> {
    log::trace!("Initializing file system watcher...");

    let watcher: Box<dyn Watcher> = if RecommendedWatcher::kind() == WatcherKind::PollWatcher {
        log::info!("Detected PollWatcher kind. Applying custom polling interval.");
        let config = Config::default()
            .with_poll_interval(Duration::from_secs(1))
            .with_compare_contents(true);

        Box::new(RecommendedWatcher::new(tx, config)?)
    } else {
        log::info!("Detected default watcher kind. Using default configuration.");
        Box::new(RecommendedWatcher::new(tx, Config::default())?)
    };

    log::debug!("File system watcher created successfully.");
    Ok(watcher)
}

use notify::{Config, Event, RecommendedWatcher, Watcher, WatcherKind};
use std::sync::mpsc;
use std::time::Duration;

pub fn create_watcher(
    tx: mpsc::Sender<Result<Event, notify::Error>>,
) -> Result<Box<dyn Watcher>, notify::Error> {
    let watcher: Box<dyn Watcher> = if RecommendedWatcher::kind() == WatcherKind::PollWatcher {
        let config = Config::default()
            .with_poll_interval(Duration::from_secs(1))
            .with_compare_contents(true);

        Box::new(RecommendedWatcher::new(tx, config)?)
    } else {
        Box::new(RecommendedWatcher::new(tx, Config::default())?)
    };

    Ok(watcher)
}

use fern::colors::{Color, ColoredLevelConfig};
use std::{io, time::SystemTime};

pub fn setup_logging(verbosity: u64) -> Result<(), fern::InitError> {
    // Base configuration for logging
    let mut base_config = fern::Dispatch::new();

    // Configure colors for log levels
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Cyan)
        .debug(Color::Green)
        .trace(Color::BrightMagenta);

    // Set log level based on verbosity
    base_config = match verbosity {
        0 => base_config
            .level(log::LevelFilter::Warn)
            .level_for("info-verbose-target", log::LevelFilter::Info),
        1 => base_config
            .level(log::LevelFilter::Info)
            .level_for("debug-verbose-target", log::LevelFilter::Debug),
        2 => base_config
            .level(log::LevelFilter::Debug)
            .level_for("trace-verbose-target", log::LevelFilter::Trace),
        _ => base_config.level(log::LevelFilter::Trace),
    };

    // Console (stdout) logging configuration
    let stdout_config = fern::Dispatch::new()
        .format(move |out, message, record| {
            // Apply colored output to stdout
            out.finish(format_args!(
                "{}{}{} {} {}",
                colors_line.color(record.level()),
                // Adjust spacing for DEBUG level logs
                if record.level().as_str().len() == 5 {
                    " "
                } else {
                    "  "
                },
                humantime::format_rfc3339_seconds(SystemTime::now()),
                ":",
                message
            ))
        })
        .chain(io::stdout()); // This sends logs to the terminal

    // Apply the logging configuration (combine file and stdout logs)
    base_config.chain(stdout_config).apply()?; // Apply the logging configuration

    Ok(())
}

use git_auto_pilot::GitAutoPilot;

mod config;
mod error;
mod git;
mod helper;
mod logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cmd_arguments = clap::Command::new("cmd-program")
        .arg(
            clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(clap::ArgAction::Count) // This is the new way to count occurrences
                .help("Increases logging verbosity each use for up to 3 times"),
        )
        .get_matches();

    // Get the number of times the verbose flag was passed
    let verbosity: u64 = cmd_arguments.get_count("verbose") as u64;

    let git_auto_pilot = GitAutoPilot::new(verbosity)?;
    GitAutoPilot::watch(git_auto_pilot).await?;
    Ok(())
}

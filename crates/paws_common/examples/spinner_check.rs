use std::thread;
use std::time::Duration;
use paws_common::spinner::SpinnerManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut spinner = SpinnerManager::new();
    let _rx = spinner.init()?;

    spinner.start(Some("Spinner started..."))?;
    thread::sleep(Duration::from_secs(2));

    spinner.write_ln("Log message 1")?;
    thread::sleep(Duration::from_secs(1));

    spinner.write_ln("Log message 2")?;
    thread::sleep(Duration::from_secs(1));

    spinner.write_ln("Log message 3")?;
    thread::sleep(Duration::from_secs(2));

    spinner.stop(Some("Done".to_string()))?;
    Ok(())
}

use std::thread;
use std::time::Duration;

use paws_common::display::md::MarkdownWriter;
use paws_common::spinner::SpinnerManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Test 1: MarkdownWriter with Spinner Active (No Height Limit) ===");
    run_test(true, None).await?;
    thread::sleep(Duration::from_secs(1));

    println!("\n=== Test 2: MarkdownWriter with Spinner Inactive (No Height Limit) ===");
    run_test(false, None).await?;
    thread::sleep(Duration::from_secs(1));

    println!("\n=== Test 3: MarkdownWriter with Spinner Active (Height Limit = 5) ===");
    run_test(true, Some(5)).await?;

    // Expected result
    println!("\n=== Test 4: Control ===");
    run_test_one_chunk().await?;
    Ok(())
}

async fn run_test_one_chunk() -> anyhow::Result<()> {
    let mut spinner = SpinnerManager::new();
    spinner.init()?;

    let mut writer = MarkdownWriter::new();

    spinner.start(Some("Processing..."))?;
    spinner.write_ln("normal text");
    spinner.write_ln("normal text 2");
    let chunks = [
        "# Header\n",
        "This is a paragraph\n",
        "built from chunks.\n",
        "- List item 1\n",
        "- List item 2\n",
        "- List item 3\n",
        "- List item 4\n",
        "- List item 5\n",
        "- List item 6\n",
        "End of stream.",
    ];

    let mut ty = String::new();

    for chunk in chunks {
        ty += chunk;
    }
    writer.add_chunk(&ty, &mut spinner);
    spinner.stop(Some("Done".to_string()))?;

    Ok(())
}

async fn run_test(use_spinner: bool, max_height: Option<usize>) -> anyhow::Result<()> {
    let mut spinner = SpinnerManager::new();
    spinner.init()?;

    let mut writer = MarkdownWriter::new();
    writer.set_max_height(max_height);

    if use_spinner {
        spinner.start(Some("Processing..."))?;
    }
    spinner.write_ln("normal text");
    spinner.write_ln("normal text 2");
    let chunks = [
        "# Header\n",
        "This is a paragraph\n",
        "built from chunks.\n",
        "- List item 1\n",
        "- List item 2\n",
        "- List item 3\n",
        "- List item 4\n",
        "- List item 5\n",
        "- List item 6\n",
        "End of stream.",
    ];

    for chunk in chunks {
        // Simulate streaming delay
        thread::sleep(Duration::from_millis(200));
        writer.add_chunk(chunk, &mut spinner);
    }

    thread::sleep(Duration::from_secs(1));

    if use_spinner {
        spinner.stop(Some("Done".to_string()))?;
    } else {
        println!("Done (no spinner)");
    }

    Ok(())
}

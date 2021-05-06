use std::error::Error;

use std::env;

use crate::{
    data::{load_preprocessed, save_mp3s},
    datatypes::ProcessedData,
};

mod data;
mod datatypes;
mod gcloud_api;
mod runner;
mod worker;

const OUTPUT_DIR: &str = "./output";
const FFMPEG_INPUT_PATH: &str = "./to-concat.txt";
const STATE_FILE: &str = "./tts-state.bin";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    dotenv::dotenv()?;

    let input_file = env::var("AO3_HTML_EXPORT_FILE").expect("AO3_HTML_EXPORT_FILE not set");

    log::info!("loading input file: {}", &input_file);
    let texts = data::load_text(&input_file).await?;

    log::info!("looking for state dump");
    let mut buffer = Vec::new();
    let processed: ProcessedData<'_> = load_preprocessed(&mut buffer, STATE_FILE).await?;

    log::info!(
        "state restored with {} previously processed items",
        processed.0.len(),
    );

    eprintln!("[1/2] Running TTS...");
    let runner = runner::Runner::new(
        processed,
        &texts.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
    );
    let data = runner.process(STATE_FILE);

    eprintln!("[2/2] Writing out MP3's...");
    save_mp3s(&texts, data, OUTPUT_DIR, FFMPEG_INPUT_PATH)?;

    eprintln!("all done! run\n\n`ffmpeg -f concat -safe 0 -i ./to_concat.txt -c copy output.mp3`\n\nto join files together :)");

    Ok(())
}

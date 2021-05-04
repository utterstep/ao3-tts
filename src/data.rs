use std::{error::Error, io::Write};

use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};
use scraper::{Html, Selector};
use tokio::{fs::File, io::AsyncReadExt};

use crate::runner::ProcessedData;

pub(crate) async fn load_text(path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut file = File::open(path).await?;
    let mut data = String::new();

    file.read_to_string(&mut data).await?;

    let html = Html::parse_document(&data);
    let selector = Selector::parse("div.userstuff p, h2.heading").expect("invalid selector");

    Ok(html
        .select(&selector)
        .filter_map(|item| {
            let text = item
                .text()
                .filter_map(|s| {
                    let s = s.trim();
                    (!s.is_empty()).then(|| s)
                })
                .collect::<Vec<_>>();

            (!text.is_empty()).then(|| text.join(" "))
        })
        .collect())
}

pub(crate) fn save_mp3s(
    texts: &[String],
    data: ProcessedData<'_>,
    output_dir: &str,
    ffmpeg_input_path: &str,
) -> Result<(), Box<dyn Error>> {
    let mut ffmpeg_input = std::fs::File::create(ffmpeg_input_path)?;
    std::fs::create_dir_all(output_dir)?;

    let progress = ProgressBar::new(texts.len() as u64);
    progress.set_style(ProgressStyle::default_bar().template(
        "[{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, ETA {eta})",
    ));

    for (idx, text) in texts.iter().enumerate().progress_with(progress) {
        let path = format!("{}/{}.mp3", output_dir, idx);

        writeln!(&mut ffmpeg_input, "file '{}'", path)?;

        let mut f = std::fs::File::create(path)?;

        let data = data.0.get(text.as_str()).expect("no data for text");
        if data.len() > 0 {
            log::info!(
                "writing out {} bytes for text of size {}",
                data.len(),
                text.len()
            );
        } else {
            log::warn!(
                "writing out {} bytes for text of size {}",
                data.len(),
                text.len()
            );
        }

        f.write_all(data)?;
    }

    Ok(())
}

pub(crate) async fn load_preprocessed<'a>(
    buffer: &'a mut Vec<u8>,
    state_file: &str,
) -> Result<ProcessedData<'a>, Box<dyn Error>> {
    log::debug!("trying to load state dump from {}", state_file);

    Ok(match File::open(state_file).await {
        Ok(mut file) => {
            log::info!("found existing state dump, loading");
            file.read_to_end(buffer).await?;

            bincode::deserialize(buffer).unwrap_or_else(|_| {
                log::warn!("invalid format in state dump, falling back to Default");
                Default::default()
            })
        }
        Err(_) => {
            log::info!("no dump present, falling back to Default");
            Default::default()
        }
    })
}

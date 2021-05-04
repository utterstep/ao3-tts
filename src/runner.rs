use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
    ops::Deref,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crossbeam::deque::Injector;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::ClientBuilder;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{rename, File},
    io::AsyncWriteExt,
    sync::mpsc::unbounded_channel,
};

use crate::worker::Worker;

#[derive(Debug, Deserialize, Serialize, Default)]
pub(crate) struct ProcessedData<'a>(#[serde(borrow)] pub BTreeMap<&'a str, Vec<u8>>);

#[derive(Debug)]
pub(crate) struct Runner<'a> {
    processed: Arc<Mutex<ProcessedData<'a>>>,
    queue: Injector<&'a str>,
    total: usize,
}

const N_WORKERS: usize = 5;

impl<'a> Runner<'a> {
    pub fn new(processed: ProcessedData<'a>, items: &[&'a str]) -> Runner<'a> {
        let queue = Injector::new();
        let mut uniqness_filter = BTreeSet::new();

        for &item in items {
            if !processed.0.contains_key(item) && uniqness_filter.insert(item) {
                queue.push(item);
            }
        }

        let preprocessed_items = processed.0.len();

        Self {
            processed: Arc::new(Mutex::new(processed)),
            queue,
            total: uniqness_filter.len() + preprocessed_items,
        }
    }

    pub fn process(self, state_path: &str) -> ProcessedData<'a> {
        // start N workers, download and write data inside them, log any errors
        // report back to owner, so he could update mapping of processed files
        // and flush it to the disk

        let (sender, mut reciever) = unbounded_channel();
        let client = ClientBuilder::new()
            .timeout(Duration::new(5, 0))
            .https_only(true)
            .build()
            .expect("failed to create reqwest Client");

        async_scoped::TokioScope::scope_and_block(|s| {
            for id in 0..N_WORKERS {
                let worker = Worker::new(client.clone(), &self.queue, sender.clone(), id);

                s.spawn(worker.do_work());
            }

            drop(sender);

            s.spawn(async {
                let progress = ProgressBar::new(self.total.try_into().unwrap());
                progress.set_style(ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, ETA {eta})"));
                progress.set_position(self.processed.lock().unwrap().0.len().try_into().unwrap());
                progress.reset_eta();

                loop {
                    let value = reciever.recv().await;

                    if let Some((text, path)) = value {
                        log::debug!("successfully processed {}, writing to {:?}", text, &path);

                        self.processed.lock().unwrap().0.insert(text, path);

                        progress.inc(1);
                    } else {
                        break;
                    }

                    if progress.position() % 25 == 0 || progress.position() == progress.length() {
                        self.dump_state(state_path).await;
                    }
                }

                progress.finish_and_clear();
            })
        });

        Arc::try_unwrap(self.processed)
            .expect("some dangling references are there")
            .into_inner()
            .expect("mutex on result is not free")
    }

    async fn dump_state(&self, state_path: &str) {
        let start_time = Instant::now();

        log::info!(
            "writing out state of size {}",
            self.processed.lock().unwrap().0.len()
        );

        // serialize
        let serialized = &bincode::serialize(self.processed.lock().unwrap().deref())
            .expect("failed to serialize state");

        // create temp file
        let temp_file = tempfile::NamedTempFile::new()
            .expect("failed to create temp file")
            .into_temp_path();

        // write out to temp file
        log::debug!("writing temp data to {}", temp_file.to_string_lossy());
        let mut state_file = File::create(&temp_file)
            .await
            .expect("failed to create state dump file");

        state_file
            .write_all(&serialized)
            .await
            .expect("failed to writeout serialized state");
        state_file.flush().await.expect("failed to flush");

        // atomically replace current dump with newly created one
        rename(&temp_file, &state_path)
            .await
            .expect("failed to rename");

        log::info!("state dump took {:?}", start_time.elapsed());
        log::info!(
            "state dumped successfully with {} items\n",
            self.processed.lock().unwrap().0.len()
        );
    }
}

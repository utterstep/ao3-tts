use std::time::{Duration, Instant};

use crossbeam::deque::{Injector, Steal, Worker as WorkerQueue};
use reqwest::Client;
use tokio::{sync::mpsc::UnboundedSender, time::sleep};

use crate::gcloud_api::GApiClient;

const ERROR_LIMIT: usize = 20;
const DURATION_ZERO: Duration = Duration::from_millis(0);
const TASK_REQUIRED_TIME: Duration = Duration::from_millis(1100);
const ERROR_BACKOFF_TIMEOUT: Duration = Duration::from_millis(5000);

pub(crate) struct Worker<'a, 'b> {
    queue: WorkerQueue<&'a str>,
    injector: &'b Injector<&'a str>,

    gapi: GApiClient,
    sender: UnboundedSender<(&'a str, Vec<u8>)>,
    id: usize,
}

impl<'a, 'b> Worker<'a, 'b> {
    pub(crate) fn new(
        client: Client,
        injector: &'b Injector<&'a str>,
        sender: UnboundedSender<(&'a str, Vec<u8>)>,
        id: usize,
    ) -> Self {
        Self {
            queue: WorkerQueue::new_fifo(),
            injector,
            sender,

            gapi: GApiClient::new(client),
            id,
        }
    }

    pub(crate) async fn do_work(self) {
        let mut errors_encountered = 0;
        let mut should_sleep = DURATION_ZERO;

        loop {
            if should_sleep > DURATION_ZERO {
                log::info!("Worker {} started to sleep", self.id);
                sleep(should_sleep).await;
                log::info!("Worker {} woke up", self.id);
            }

            let next_job = self.queue.pop();

            let job = match next_job {
                Some(job) => job,
                None => {
                    log::debug!("Worker {} queue is empty, attempting to steal", self.id);

                    match self.injector.steal_batch_and_pop(&self.queue) {
                        Steal::Retry => continue,
                        Steal::Empty => break,
                        Steal::Success(value) => {
                            log::debug!("Worker {} stealed job successfuly", self.id);

                            value
                        }
                    }
                }
            };

            log::debug!("Worker {} started working on {}", self.id, job);
            log::info!("Worker {} started new job working", self.id);

            let start_time = Instant::now();
            let result = self.gapi.generate_text(job).await;
            let elapsed = start_time.elapsed();
            should_sleep = TASK_REQUIRED_TIME
                .checked_sub(elapsed)
                .unwrap_or(DURATION_ZERO);

            log::debug!(
                "Request from worker {} took {:?}. Will sleep for {:?}",
                self.id,
                elapsed,
                should_sleep,
            );

            let data = match result {
                Ok(data) => data,
                Err(e) => {
                    errors_encountered += 1;
                    should_sleep = ERROR_BACKOFF_TIMEOUT;

                    if errors_encountered <= ERROR_LIMIT {
                        log::warn!(
                            "Worker {} got error #{} while calling TTS: {}. Will sleep for {:?}",
                            self.id,
                            errors_encountered,
                            e,
                            should_sleep,
                        );

                        self.injector.push(job);
                    } else {
                        log::error!(
                            "Worker {} got {} errors, aborting! Last one is: {}",
                            self.id,
                            errors_encountered,
                            e
                        );
                        panic!("aborting on {}", e);
                    }

                    continue;
                }
            };

            log::debug!("Worker {} done working on {}", self.id, job);
            log::debug!("Worker {} done another job", self.id);

            match self.sender.send((job, data)) {
                Ok(()) => {}
                Err(e) => {
                    log::error!("Initiating system shutdown due to failed message send");
                    panic!("Worker {} got error while sending data: {}", self.id, e);
                }
            };
        }
    }
}

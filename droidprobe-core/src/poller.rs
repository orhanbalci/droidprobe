//! Recurring execution. A [`Poller`] spawns one tokio task per registered poll
//! job; each job sleeps for its interval, runs its command through the
//! [`Engine`], and broadcasts a [`PollUpdate`]. The TUI subscribes and repaints
//! when updates arrive; the MCP server generally doesn't need polling and can
//! ignore this module entirely.

use std::time::Duration;

use serde::Serialize;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use droidprobe_command::Json;

use crate::engine::Engine;

/// A single poll result published to subscribers.
#[derive(Debug, Clone, Serialize)]
pub struct PollUpdate {
    /// The command id that produced this update.
    pub command_id: String,
    /// Serial of the device polled, if any.
    pub serial: Option<String>,
    /// `Ok(json)` output or `Err(message)` if the run failed.
    pub result: Result<Json, String>,
}

/// Handle to a running poll task; dropping it does not stop the task — call
/// [`PollHandle::abort`] for that.
pub struct PollHandle {
    pub command_id: String,
    handle: JoinHandle<()>,
}

impl PollHandle {
    pub fn abort(&self) {
        self.handle.abort();
    }
}

/// Manages a set of recurring poll jobs over a shared broadcast channel.
pub struct Poller {
    engine: Engine,
    tx: broadcast::Sender<PollUpdate>,
    handles: Vec<PollHandle>,
}

impl Poller {
    pub fn new(engine: Engine, channel_capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(channel_capacity);
        Self {
            engine,
            tx,
            handles: Vec::new(),
        }
    }

    /// Subscribe to updates from all poll jobs.
    pub fn subscribe(&self) -> broadcast::Receiver<PollUpdate> {
        self.tx.subscribe()
    }

    /// Begin polling `command_id` every `interval`, optionally with fixed
    /// `args` and a target `serial`. Returns immediately; work happens in a
    /// background task. An initial run fires right away (no leading delay) so
    /// the UI has data fast.
    pub fn add_job(
        &mut self,
        command_id: impl Into<String>,
        serial: Option<String>,
        args: Json,
        interval: Duration,
    ) {
        let command_id = command_id.into();
        let engine = self.engine.clone();
        let tx = self.tx.clone();
        let job_id = command_id.clone();

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // Tick immediately, then on the interval. Skip missed ticks rather
            // than bursting if a run runs long.
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let result = engine
                    .run(&job_id, serial.as_deref(), args.clone())
                    .await
                    .map_err(|e| e.to_string());

                // If all receivers have dropped, stop the job.
                if tx
                    .send(PollUpdate {
                        command_id: job_id.clone(),
                        serial: serial.clone(),
                        result,
                    })
                    .is_err()
                {
                    tracing::debug!(command = %job_id, "no subscribers; stopping poll job");
                    break;
                }
            }
        });

        self.handles.push(PollHandle { command_id, handle });
    }

    /// Abort every running job.
    pub fn stop_all(&mut self) {
        for h in &self.handles {
            h.abort();
        }
        self.handles.clear();
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        self.stop_all();
    }
}

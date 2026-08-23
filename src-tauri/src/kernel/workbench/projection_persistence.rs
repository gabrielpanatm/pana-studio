use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager, Runtime};

use crate::{kernel::project_session::ProjectSessionSnapshot, state::AppState};

use super::{persist_latest_workbench, WorkbenchSnapshot};

const PROJECTION_PERSISTENCE_QUIET_PERIOD: Duration = Duration::from_millis(250);

#[derive(Clone, Debug)]
struct PendingProjection {
    session: ProjectSessionSnapshot,
    snapshot: WorkbenchSnapshot,
}

enum DebouncePoll<T> {
    Idle,
    Wait(Duration),
    Ready(T),
}

struct DebouncedLatest<T> {
    pending: Option<T>,
    deadline: Option<Instant>,
    worker_running: bool,
}

impl<T> Default for DebouncedLatest<T> {
    fn default() -> Self {
        Self {
            pending: None,
            deadline: None,
            worker_running: false,
        }
    }
}

impl<T> DebouncedLatest<T> {
    fn replace(&mut self, value: T, now: Instant, quiet_period: Duration) -> bool {
        self.pending = Some(value);
        self.deadline = Some(now + quiet_period);
        if self.worker_running {
            false
        } else {
            self.worker_running = true;
            true
        }
    }

    fn poll(&mut self, now: Instant) -> DebouncePoll<T> {
        let Some(_) = self.pending else {
            self.deadline = None;
            self.worker_running = false;
            return DebouncePoll::Idle;
        };
        let deadline = self
            .deadline
            .expect("pending Workbench projection has a deadline");
        if now < deadline {
            return DebouncePoll::Wait(deadline.saturating_duration_since(now));
        }
        self.deadline = None;
        DebouncePoll::Ready(
            self.pending
                .take()
                .expect("pending Workbench projection was checked"),
        )
    }
}

#[derive(Default)]
pub struct WorkbenchProjectionPersistence {
    queue: Mutex<DebouncedLatest<PendingProjection>>,
}

impl WorkbenchProjectionPersistence {
    pub fn schedule<R: Runtime>(
        &self,
        app: AppHandle<R>,
        session: ProjectSessionSnapshot,
        snapshot: WorkbenchSnapshot,
    ) -> Result<(), String> {
        let start_worker = self
            .queue
            .lock()
            .map_err(|_| "Coada de persistență Workbench este compromisă.".to_string())?
            .replace(
                PendingProjection { session, snapshot },
                Instant::now(),
                PROJECTION_PERSISTENCE_QUIET_PERIOD,
            );
        if start_worker {
            spawn_projection_persistence_worker(app);
        }
        Ok(())
    }

    pub fn flush_latest<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        session: &ProjectSessionSnapshot,
        snapshot: &WorkbenchSnapshot,
    ) -> Result<(), String> {
        // persist_latest_workbench serializes with an already-running worker
        // and skips the worker's queued copy when this exact revision wins.
        persist_latest_workbench(app, session, snapshot)
    }

    fn poll(&self, now: Instant) -> Result<DebouncePoll<PendingProjection>, String> {
        self.queue
            .lock()
            .map_err(|_| "Coada de persistență Workbench este compromisă.".to_string())
            .map(|mut queue| queue.poll(now))
    }
}

fn spawn_projection_persistence_worker<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        loop {
            let poll = {
                let state = app.state::<AppState>();
                state.workbench_projection_persistence.poll(Instant::now())
            };
            match poll {
                Ok(DebouncePoll::Idle) => return,
                Ok(DebouncePoll::Wait(delay)) => tokio::time::sleep(delay).await,
                Ok(DebouncePoll::Ready(pending)) => {
                    let persist_app = app.clone();
                    let revision = pending.snapshot.revision;
                    let result = tauri::async_runtime::spawn_blocking(move || {
                        persist_latest_workbench(&persist_app, &pending.session, &pending.snapshot)
                    })
                    .await;
                    match result {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => eprintln!(
                            "[Pană Studio] Workbench projection write-behind failed at revision {revision}: {error}"
                        ),
                        Err(error) => eprintln!(
                            "[Pană Studio] Workbench projection write-behind worker failed at revision {revision}: {error}"
                        ),
                    }
                }
                Err(error) => {
                    eprintln!("[Pană Studio] {error}");
                    return;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_burst_keeps_one_worker_and_only_the_latest_projection() {
        let mut queue = DebouncedLatest::default();
        let started_at = Instant::now();

        for revision in 1..=203_u64 {
            let starts_worker = queue.replace(
                revision,
                started_at + Duration::from_millis(revision),
                PROJECTION_PERSISTENCE_QUIET_PERIOD,
            );
            assert_eq!(starts_worker, revision == 1);
        }

        assert!(matches!(
            queue.poll(started_at + Duration::from_millis(452)),
            DebouncePoll::Wait(_)
        ));
        assert!(matches!(
            queue.poll(started_at + Duration::from_millis(453)),
            DebouncePoll::Ready(203)
        ));
        assert!(matches!(
            queue.poll(started_at + Duration::from_millis(453)),
            DebouncePoll::Idle
        ));
    }
}

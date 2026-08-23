use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use crate::project::AcceptedProjectDiskManifest;

use super::VersionNetworkOperationKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VersionNetworkOperationLease {
    pub(crate) operation_id: String,
    pub(crate) project_root: String,
    pub(crate) session_id: String,
    pub(crate) kind: VersionNetworkOperationKind,
    pub(crate) workspace_revision: u64,
    pub(crate) disk_generation: u64,
    pub(crate) accepted_disk: Arc<AcceptedProjectDiskManifest>,
    pub(crate) expected_status_token: String,
    pub(crate) expected_head_oid: Option<String>,
}

#[derive(Clone)]
struct ActiveVersionNetworkOperation {
    lease: VersionNetworkOperationLease,
    cancellation: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct VersionNetworkOperationRuntime {
    active: Mutex<Option<ActiveVersionNetworkOperation>>,
}

impl VersionNetworkOperationRuntime {
    pub(crate) fn begin(
        &self,
        lease: VersionNetworkOperationLease,
        cancellation: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let mut active = self.lock_active()?;
        if let Some(active) = active.as_ref() {
            return Err(format!(
                "Operația Git de rețea {} este deja activă.",
                active.lease.operation_id
            ));
        }
        *active = Some(ActiveVersionNetworkOperation {
            lease,
            cancellation,
        });
        Ok(())
    }

    pub(crate) fn require_source_mutation_allowed(
        &self,
        operation: &str,
        project_root: &str,
        session_id: &str,
    ) -> Result<(), String> {
        let active = self.lock_active()?;
        let Some(active) = active.as_ref() else {
            return Ok(());
        };
        let scope =
            if active.lease.project_root == project_root && active.lease.session_id == session_id {
                "sesiunea curentă"
            } else {
                "altă sesiune activă"
            };
        Err(format!(
            "{operation} este blocată rapid de operația Git remote {} ({}) pentru {scope}; anulează sau așteaptă finalizarea ei.",
            active.lease.operation_id,
            operation_kind_label(active.lease.kind),
        ))
    }

    pub(crate) fn require_git_mutation_allowed(&self, operation: &str) -> Result<(), String> {
        let active = self.lock_active()?;
        let Some(active) = active.as_ref() else {
            return Ok(());
        };
        Err(format!(
            "{operation} este blocată rapid de operația Git remote {} ({}); anulează sau așteaptă finalizarea ei.",
            active.lease.operation_id,
            operation_kind_label(active.lease.kind),
        ))
    }

    pub(crate) fn require_project_transition_allowed(&self) -> Result<(), String> {
        let active = self.lock_active()?;
        let Some(active) = active.as_ref() else {
            return Ok(());
        };
        Err(format!(
            "Project Transition este blocat rapid de operația Git remote {} ({}) pentru sesiunea {}; anulează sau așteaptă finalizarea ei.",
            active.lease.operation_id,
            operation_kind_label(active.lease.kind),
            active.lease.session_id,
        ))
    }

    pub(crate) fn require_current(
        &self,
        expected: &VersionNetworkOperationLease,
    ) -> Result<(), String> {
        let active = self.lock_active()?;
        let Some(active) = active.as_ref() else {
            return Err(format!(
                "Operația Git remote {} nu mai este activă la publicare.",
                expected.operation_id
            ));
        };
        if &active.lease != expected {
            return Err(format!(
                "Operația Git remote {} a devenit stale înainte de publicare.",
                expected.operation_id
            ));
        }
        if active.cancellation.load(Ordering::SeqCst) {
            return Err("Operația Git de rețea a fost anulată.".to_string());
        }
        Ok(())
    }

    pub(crate) fn request_cancellation(
        &self,
        operation_id: &str,
        project_root: &str,
        session_id: &str,
    ) -> Result<Option<VersionNetworkOperationLease>, String> {
        let active = self.lock_active()?;
        let Some(active) = active.as_ref() else {
            return Ok(None);
        };
        if active.lease.operation_id != operation_id {
            return Err(format!(
                "Operația Git activă este {}, nu {operation_id}.",
                active.lease.operation_id
            ));
        }
        if active.lease.project_root != project_root || active.lease.session_id != session_id {
            return Err(
                "Anularea Git a fost refuzată deoarece proiectul sau sesiunea nu corespund."
                    .to_string(),
            );
        }
        active.cancellation.store(true, Ordering::SeqCst);
        Ok(Some(active.lease.clone()))
    }

    pub(crate) fn finish_success(
        &self,
        expected: &VersionNetworkOperationLease,
    ) -> Result<(), String> {
        let mut active = self.lock_active()?;
        let Some(live) = active.as_ref() else {
            return Err(format!(
                "Operația Git remote {} nu mai este activă la finalizare.",
                expected.operation_id
            ));
        };
        if &live.lease != expected {
            return Err(format!(
                "Operația Git remote {} a devenit stale la finalizare.",
                expected.operation_id
            ));
        }
        if live.cancellation.load(Ordering::SeqCst) {
            return Err("Operația Git de rețea a fost anulată.".to_string());
        }
        *active = None;
        Ok(())
    }

    pub(crate) fn abandon(&self, operation_id: &str) {
        let Ok(mut active) = self.active.lock() else {
            eprintln!("[Pană Studio] Runtime-ul operației Git remote este compromis la cleanup.");
            return;
        };
        if active
            .as_ref()
            .is_some_and(|active| active.lease.operation_id == operation_id)
        {
            *active = None;
        }
    }

    fn lock_active(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<ActiveVersionNetworkOperation>>, String> {
        self.active
            .lock()
            .map_err(|_| "Runtime-ul operației Git remote este compromis.".to_string())
    }
}

pub(crate) fn execute_version_network_phases<Prepared, Executed, Receipt>(
    capture: impl FnOnce() -> Result<Prepared, String>,
    execute: impl FnOnce(Prepared) -> Result<Executed, String>,
    publish: impl FnOnce(Executed) -> Result<Receipt, String>,
) -> Result<Receipt, String> {
    let prepared = capture()?;
    let executed = execute(prepared)?;
    publish(executed)
}

fn operation_kind_label(kind: VersionNetworkOperationKind) -> &'static str {
    match kind {
        VersionNetworkOperationKind::Fetch => "fetch",
        VersionNetworkOperationKind::Push => "push",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{AcceptedProjectDiskManifest, ProjectDiskManifest};
    use std::{
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    fn lease(operation_id: &str, session_id: &str) -> VersionNetworkOperationLease {
        let root = "/tmp/pana-version-network-runtime".to_string();
        VersionNetworkOperationLease {
            operation_id: operation_id.to_string(),
            project_root: root.clone(),
            session_id: session_id.to_string(),
            kind: VersionNetworkOperationKind::Fetch,
            workspace_revision: 7,
            disk_generation: 3,
            accepted_disk: Arc::new(
                AcceptedProjectDiskManifest::new(
                    session_id,
                    &root,
                    ProjectDiskManifest {
                        root: root.clone(),
                        files: Vec::new(),
                        truncated: false,
                        max_files: 1_000,
                    },
                )
                .unwrap(),
            ),
            expected_status_token: "status-before".to_string(),
            expected_head_oid: Some("head-before".to_string()),
        }
    }

    #[test]
    fn active_operation_rejects_mutations_and_transition_without_consuming_the_lease() {
        let runtime = VersionNetworkOperationRuntime::default();
        let lease = lease("fetch-12345678", "session-a");
        runtime
            .begin(lease.clone(), Arc::new(AtomicBool::new(false)))
            .unwrap();

        assert!(runtime
            .require_source_mutation_allowed(
                "Save ProjectWorkspace",
                &lease.project_root,
                &lease.session_id
            )
            .unwrap_err()
            .contains("fetch-12345678"));
        assert!(runtime
            .require_git_mutation_allowed("Restore")
            .unwrap_err()
            .contains("fetch-12345678"));
        assert!(runtime
            .require_project_transition_allowed()
            .unwrap_err()
            .contains("fetch-12345678"));
        runtime.require_current(&lease).unwrap();
        runtime.finish_success(&lease).unwrap();
        assert!(runtime
            .finish_success(&lease)
            .unwrap_err()
            .contains("nu mai este activă"));
    }

    #[test]
    fn cancellation_invalidates_success_and_cleanup_is_idempotent() {
        let runtime = VersionNetworkOperationRuntime::default();
        let lease = lease("push-12345678", "session-a");
        runtime
            .begin(lease.clone(), Arc::new(AtomicBool::new(false)))
            .unwrap();

        let cancelled = runtime
            .request_cancellation(&lease.operation_id, &lease.project_root, &lease.session_id)
            .unwrap()
            .unwrap();
        assert_eq!(cancelled, lease);
        assert!(runtime
            .require_current(&lease)
            .unwrap_err()
            .contains("anulată"));
        assert!(runtime
            .finish_success(&lease)
            .unwrap_err()
            .contains("anulată"));
        runtime.abandon(&lease.operation_id);
        runtime.abandon(&lease.operation_id);
        runtime
            .begin(
                super::tests::lease("fetch-87654321", "session-b"),
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
    }

    #[test]
    fn reopened_same_root_is_a_different_lease() {
        let runtime = VersionNetworkOperationRuntime::default();
        let original = lease("fetch-12345678", "session-a");
        runtime
            .begin(original.clone(), Arc::new(AtomicBool::new(false)))
            .unwrap();

        let reopened = lease("fetch-12345678", "session-b");
        assert!(runtime
            .require_current(&reopened)
            .unwrap_err()
            .contains("stale"));
        runtime.abandon(&original.operation_id);
    }

    #[test]
    fn slow_remote_execute_phase_does_not_hold_the_workspace_mutex() {
        let workspace = Arc::new(Mutex::new(()));
        let worker_workspace = Arc::clone(&workspace);
        let (executing_tx, executing_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let worker = thread::spawn(move || {
            execute_version_network_phases(
                || {
                    let _workspace_guard = worker_workspace.lock().unwrap();
                    Ok("prepared")
                },
                |prepared| {
                    executing_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(prepared)
                },
                |executed| Ok(executed),
            )
            .unwrap()
        });

        executing_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let started = Instant::now();
        let read_guard = workspace
            .try_lock()
            .expect("ProjectWorkspace trebuie să fie disponibil cât executorul remote este blocat");
        assert!(started.elapsed() < Duration::from_millis(50));
        drop(read_guard);
        release_tx.send(()).unwrap();
        assert_eq!(worker.join().unwrap(), "prepared");
    }
}

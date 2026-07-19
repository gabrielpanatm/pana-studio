use std::sync::{Mutex, MutexGuard};

// Pană Studio menține o singură sesiune de proiect activă. Retention-ul normal
// și recovery-ul aceleiași familii formează o singură operație de domeniu și nu
// au voie să ruleze simultan în proces. Filesystem CAS rămâne autoritatea pentru
// procese/executabile externe; acest mutex elimină cursa locală hot-write/clear.
static PROJECT_TRANSITION_DECISION_RETENTION_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn lock_project_transition_decision_retention() -> MutexGuard<'static, ()> {
    PROJECT_TRANSITION_DECISION_RETENTION_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

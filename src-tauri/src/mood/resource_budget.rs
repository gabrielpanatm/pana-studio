use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};

pub(crate) const MOOD_ASSET_RESOURCE_BUSY_DIAGNOSTIC: &str =
    "O operație raster Mood Board este deja activă; cererea repetată a fost refuzată pentru a proteja resursele aplicației.";

#[derive(Clone, Debug)]
struct MoodAssetResourceBudget {
    active: Arc<AtomicBool>,
}

impl MoodAssetResourceBudget {
    fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    fn try_acquire(&self) -> Result<MoodAssetResourcePermit, String> {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| MOOD_ASSET_RESOURCE_BUSY_DIAGNOSTIC.to_string())?;
        Ok(MoodAssetResourcePermit {
            active: Arc::clone(&self.active),
        })
    }
}

#[derive(Debug)]
#[must_use = "permit-ul trebuie păstrat pe toată durata operației raster"]
pub(crate) struct MoodAssetResourcePermit {
    active: Arc<AtomicBool>,
}

impl Drop for MoodAssetResourcePermit {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

pub(crate) fn acquire_heavy_mood_asset_operation() -> Result<MoodAssetResourcePermit, String> {
    static BUDGET: OnceLock<MoodAssetResourceBudget> = OnceLock::new();
    BUDGET
        .get_or_init(MoodAssetResourceBudget::new)
        .try_acquire()
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, thread, time::Duration};

    use super::*;

    #[test]
    fn permit_is_fail_fast_and_releases_on_drop() {
        let budget = MoodAssetResourceBudget::new();
        let permit = budget.try_acquire().expect("first operation owns budget");

        assert_eq!(
            budget.try_acquire().unwrap_err(),
            MOOD_ASSET_RESOURCE_BUSY_DIAGNOSTIC
        );

        drop(permit);
        assert!(budget.try_acquire().is_ok());
    }

    #[test]
    fn cloned_budget_shares_the_same_single_operation_limit() {
        let budget = MoodAssetResourceBudget::new();
        let cloned = budget.clone();
        let _permit = budget.try_acquire().expect("first operation owns budget");

        assert_eq!(
            cloned.try_acquire().unwrap_err(),
            MOOD_ASSET_RESOURCE_BUSY_DIAGNOSTIC
        );
    }

    #[test]
    fn permit_moved_to_worker_holds_budget_until_worker_releases_it() {
        let budget = MoodAssetResourceBudget::new();
        let permit = budget.try_acquire().expect("worker permit");
        let (release_tx, release_rx) = mpsc::channel();
        let (dropped_tx, dropped_rx) = mpsc::channel();

        let worker = thread::spawn(move || {
            release_rx.recv().expect("release signal");
            drop(permit);
            dropped_tx.send(()).expect("drop signal");
        });

        assert_eq!(
            budget.try_acquire().unwrap_err(),
            MOOD_ASSET_RESOURCE_BUSY_DIAGNOSTIC
        );
        release_tx.send(()).expect("release worker");
        dropped_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("worker released permit");
        assert!(budget.try_acquire().is_ok());
        worker.join().expect("worker completed");
    }
}

use std::{
    any::Any,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::Mutex,
};

/// The official Zola engine uses process-global state (`SITE_CONTENT`) and
/// internal shared caches. Every Pană Studio consumer must enter through this
/// authority so Preview, Source Browser, validation and production builds can
/// never mutate that state concurrently.
static ZOLA_ENGINE_AUTHORITY: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) struct ZolaEngineGuard {
    _guard: std::sync::MutexGuard<'static, ()>,
}

pub(crate) const EMBEDDED_ZOLA_VERSION: &str = "0.22.1";
pub(crate) const EMBEDDED_ZOLA_REVISION: &str = "29540e9897dbe8aca388b13f7bdf615985f6ca2c";

pub(crate) fn with_zola_engine<T>(
    operation: &str,
    execute: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let guard = match ZOLA_ENGINE_AUTHORITY.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!(
                "[Pană Studio] Autoritatea Zola embedded a fost recuperată după un panic anterior."
            );
            poisoned.into_inner()
        }
    };
    let result = catch_unwind(AssertUnwindSafe(execute));
    drop(guard);

    match result {
        Ok(result) => result,
        Err(payload) => Err(format!(
            "Motorul Zola embedded {EMBEDDED_ZOLA_VERSION} a întâmpinat un panic izolat în timpul operației «{operation}»: {}",
            panic_payload(payload.as_ref())
        )),
    }
}

#[cfg(test)]
pub(crate) fn acquire_zola_engine_for_test() -> ZolaEngineGuard {
    let guard = ZOLA_ENGINE_AUTHORITY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ZolaEngineGuard { _guard: guard }
}

pub(crate) fn zola_config_file(root: &Path) -> Result<PathBuf, String> {
    for name in ["zola.toml", "config.toml"] {
        let path = root.join(name);
        if path.is_file() {
            return Ok(PathBuf::from(name));
        }
    }
    Err(format!(
        "Proiectul Zola nu conține zola.toml sau config.toml: {}.",
        root.display()
    ))
}

fn panic_payload(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic fără mesaj".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, thread, time::Duration};

    #[test]
    fn authority_is_reusable_after_an_isolated_engine_panic() {
        let error = with_zola_engine::<()>("test panic", || panic!("engine panic")).unwrap_err();
        assert!(error.contains("engine panic"));
        assert_eq!(with_zola_engine("test recovery", || Ok(7)).unwrap(), 7);
    }

    #[test]
    fn embedded_revision_is_pinned_explicitly() {
        assert_eq!(EMBEDDED_ZOLA_VERSION, "0.22.1");
        assert_eq!(
            EMBEDDED_ZOLA_REVISION,
            "29540e9897dbe8aca388b13f7bdf615985f6ca2c"
        );
    }

    #[test]
    fn authority_serializes_independent_engine_consumers() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let first = thread::spawn(move || {
            with_zola_engine("first", || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            })
            .unwrap();
        });
        entered_rx.recv().unwrap();

        let (second_tx, second_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            with_zola_engine("second", || {
                second_tx.send(()).unwrap();
                Ok(())
            })
            .unwrap();
        });
        assert!(second_rx.recv_timeout(Duration::from_millis(50)).is_err());
        release_tx.send(()).unwrap();
        first.join().unwrap();
        second_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        second.join().unwrap();
    }
}

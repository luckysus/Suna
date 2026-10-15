use crate::storage::repositories::literature_discovery as repo;
use rusqlite::Connection;

pub(super) fn store_page(
    connection: &mut Connection,
    workspace: &str,
    records: Vec<repo::Metadata>,
) -> Result<Option<Vec<repo::Record>>, String> {
    // No network or await occurs under this bounded, page-level write lock.
    // Acquire it before the first read so concurrent scheduler checkpoints
    // cannot invalidate a deferred WAL snapshot before it becomes a writer.
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if !repo::online(&transaction, workspace)? {
        return Ok(None);
    }
    let saved = records
        .into_iter()
        .map(|record| repo::store(&transaction, workspace, record))
        .collect::<Result<Vec<_>, _>>()?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(Some(saved))
}

pub(super) fn handler_error(error: String) -> crate::tasks::scheduler::HandlerError {
    use crate::tasks::scheduler::HandlerError;
    if matches!(
        error.as_str(),
        "database is locked" | "database is busy" | "database table is locked"
    ) {
        HandlerError::retryable("literature_storage_busy")
    } else {
        HandlerError::permanent(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::repositories::settings;
    use crate::tasks::{repository, NewTask};
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    };
    use std::time::{Duration, Instant};

    static RELEASE: AtomicBool = AtomicBool::new(false);
    static BUSY_CALLS: AtomicUsize = AtomicUsize::new(0);
    fn release_writer_on_busy(attempt: i32) -> bool {
        BUSY_CALLS.fetch_add(1, Ordering::SeqCst);
        RELEASE.store(true, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(2));
        attempt < 500
    }

    #[test]
    fn concurrent_scheduler_write_does_not_invalidate_metadata_snapshot_or_leave_partial_pages() {
        RELEASE.store(false, Ordering::SeqCst);
        BUSY_CALLS.store(0, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "suna-literature-lock-中文-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let database = root.join("fixture.sqlite3");
        let (mut c, _) = crate::storage::database::open(&database).unwrap();
        c.pragma_update(None, "journal_mode", "WAL").unwrap();
        settings::set(&mut c, "local", repo::NETWORK_KEY, "true").unwrap();
        let task = repository::create(
            &c,
            NewTask {
                workspace_id: "local".into(),
                kind: "synthetic_progress".into(),
                payload_json: "{}".into(),
                checkpoint_json: None,
                next_run_at: None,
                progress: 0,
            },
        )
        .unwrap();
        c.busy_handler(Some(release_writer_on_busy)).unwrap();
        let (ready, wait) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            let (mut c, _) = crate::storage::database::open(&database).unwrap();
            let tx = c
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            tx.execute(
                "UPDATE background_tasks SET progress=1 WHERE id=?1",
                [task.id.to_string()],
            )
            .unwrap();
            ready.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !RELEASE.load(Ordering::SeqCst) {
                assert!(Instant::now() < deadline, "fixture writer release deadline");
                std::thread::sleep(Duration::from_millis(1));
            }
            tx.commit().unwrap();
        });
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
        let metadata = repo::Metadata {
            provider: "crossref".into(),
            external_id: "10.1234/lock-fixture".into(),
            doi: Some("10.1234/lock-fixture".into()),
            title: "Synthetic lock regression".into(),
            authors: vec![],
            journal: None,
            year: None,
            abstract_text: None,
            source_url: "https://doi.org/10.1234/lock-fixture".into(),
            license: None,
            fulltext_state: "link_only".into(),
        };
        let result = store_page(&mut c, "local", vec![metadata.clone()]);
        RELEASE.store(true, Ordering::SeqCst);
        writer.join().unwrap();
        assert!(result.is_ok(), "{:?}", result.as_ref().err());
        assert!(
            BUSY_CALLS.load(Ordering::SeqCst) > 0,
            "must acquire the write transaction before reading a snapshot"
        );
        assert_eq!(result.unwrap().unwrap().len(), 1);
        let mut another = metadata.clone();
        another.doi = Some("10.1234/another-lock-fixture".into());
        let mut invalid = metadata;
        invalid.title.clear();
        assert!(store_page(&mut c, "local", vec![another.clone(), invalid]).is_err());
        assert_eq!(repo::list(&c, "local", "", 0).unwrap().len(), 1);
        settings::set(&mut c, "local", repo::NETWORK_KEY, "false").unwrap();
        assert!(store_page(&mut c, "local", vec![another])
            .unwrap()
            .is_none());
        assert_eq!(repo::list(&c, "local", "", 0).unwrap().len(), 1);
        for error in [
            "database is locked",
            "database is busy",
            "database table is locked",
        ] {
            let mapped = handler_error(error.into());
            assert!(mapped.is_retryable());
            assert_eq!(mapped.code(), "literature_storage_busy");
        }
        assert!(!handler_error("invalid_literature_metadata".into()).is_retryable());
        drop(c);
        std::fs::remove_dir_all(root).unwrap();
    }
}

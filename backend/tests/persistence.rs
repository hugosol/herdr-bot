//! Behavioral tests at the FileStore/FileAccess seam. Raw fs only seeds fixtures.
mod common;
use common::TempDir;
use herdr_bot::files::{FileStore, PublishMode};
use serde_json::{Value, json};
use std::{fs, future::{Future, poll_fn}, io, pin::Pin, task::Poll, time::Duration};
#[cfg(windows)]
use std::path::PathBuf;

async fn assert_pending<F: Future>(mut future: Pin<&mut F>) {
    poll_fn(|context| {
        assert!(future.as_mut().poll(context).is_pending(), "overlapping scope entered before its owner released it");
        Poll::Ready(())
    }).await;
}

#[tokio::test]
async fn directory_scope_excludes_descendants_across_stores_but_not_siblings() {
    let root = TempDir::new("persistence overlap");
    let directory = root.path().join("run");
    let child = directory.join("events.jsonl");
    fs::create_dir(&directory).unwrap();
    fs::write(&child, b"before").unwrap();
    let first = FileStore::default();
    let second = FileStore::default();
    let (release, released) = tokio::sync::oneshot::channel();
    let owner_paths = [directory.clone()];
    let owner_child = child.clone();
    let owner = first.with_async(&owner_paths, move |access| Box::pin(async move {
        released.await.unwrap();
        access.replace(&owner_child, b"after")
    }));
    tokio::pin!(owner);
    assert_pending(owner.as_mut()).await;

    // The lexical dot/parent alias names exactly the same child.
    let alias = directory.join(".").join("unused").join("..").join("events.jsonl");
    let waiting_paths = [alias];
    let waiting_child = child.clone();
    let waiting = second.with_async(&waiting_paths, move |access| Box::pin(async move {
        let previous = access.read(&waiting_child)?;
        access.replace(waiting_child, b"child mutation")?;
        Ok::<_, io::Error>(previous)
    }));
    tokio::pin!(waiting);
    assert_pending(waiting.as_mut()).await;

    let independent = root.path().join("run-other");
    second.with(&[independent.clone()], |access| access.create(&independent, b"independent")).unwrap().unwrap();
    release.send(()).unwrap();
    owner.await.unwrap().unwrap();
    assert_eq!(waiting.await.unwrap().unwrap(), b"after");
    assert_eq!(second.read(child).unwrap(), b"child mutation");
}

#[tokio::test]
async fn synchronous_owner_and_async_waiter_share_coordination() {
    let root = TempDir::new("persistence sync async");
    let path = root.path().join("record.json");
    fs::write(&path, b"old").unwrap();
    let (entered, entering) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let owner_path = path.clone();
    let owner = std::thread::spawn(move || {
        FileStore::default().with(&[owner_path.clone()], |access| {
            entered.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(10)).unwrap();
            access.replace(owner_path, b"new")
        }).unwrap().unwrap();
    });
    entering.recv_timeout(Duration::from_secs(10)).unwrap();
    let store = FileStore::default();
    let paths = [path.clone()];
    let waiting = store.with_async(&paths, move |access| Box::pin(async move { access.read(path) }));
    tokio::pin!(waiting);
    assert_pending(waiting.as_mut()).await;
    release.send(()).unwrap();
    assert_eq!(waiting.await.unwrap().unwrap(), b"new");
    owner.join().unwrap();
}

#[tokio::test]
async fn multi_path_acquisition_never_reserves_an_unblocked_subset() {
    let root = TempDir::new("persistence atomic acquisition");
    let a = root.path().join("a");
    let b = root.path().join("b");
    let store = FileStore::default();
    let (release, released) = tokio::sync::oneshot::channel();
    let owner_paths = [a.clone()];
    let owner = store.with_async(&owner_paths, move |_| Box::pin(async move { released.await.unwrap(); }));
    tokio::pin!(owner);
    assert_pending(owner.as_mut()).await;
    let waiting_paths = [b.clone(), a.clone(), b.clone()];
    let waiting = store.with_async(&waiting_paths, |_| Box::pin(async {}));
    tokio::pin!(waiting);
    assert_pending(waiting.as_mut()).await;
    // A waiter for [b,a] must not reserve b while a is unavailable.
    let other_paths = [b.clone()];
    let other = store.with_async(&other_paths, move |access| Box::pin(async move { access.create(b, b"free") }));
    tokio::pin!(other);
    poll_fn(|context| match other.as_mut().poll(context) {
        Poll::Ready(result) => { result.unwrap().unwrap(); Poll::Ready(()) },
        Poll::Pending => panic!("multi-path waiter held an unblocked subset"),
    }).await;
    release.send(()).unwrap();
    owner.await.unwrap();
    waiting.await.unwrap();
}

#[tokio::test]
async fn cancellation_and_panics_release_owned_scopes() {
    let root = TempDir::new("persistence scope release");
    let path = root.path().join("value");
    let store = FileStore::default();
    let paths = [path.clone()];
    let mut owner = Box::pin(store.with_async(&paths, |_| Box::pin(std::future::pending::<()>() )));
    assert_pending(owner.as_mut()).await;
    drop(owner);
    store.with(&paths, |access| access.create(&path, b"released")).unwrap().unwrap();
    let panic = std::panic::catch_unwind(|| {
        let _ = store.with(&paths, |_| panic!("callback failed"));
    });
    assert!(panic.is_err());
    assert_eq!(store.read(&path).unwrap(), b"released");
}

#[test]
fn scopes_reject_outside_content_but_allow_readonly_ancestor_validation() {
    let root = TempDir::new("persistence capability");
    let path = root.path().join("record");
    let outside = root.path().join("outside");
    fs::write(&path, b"inside").unwrap();
    fs::write(&outside, b"outside").unwrap();
    FileStore::default().with(&[path.clone()], |access| {
        access.symlink_metadata(root.path()).unwrap();
        access.canonicalize(root.path()).unwrap();
        for error in [access.read(&outside).unwrap_err(), access.read_dir(root.path()).err().unwrap(),
            access.remove_dir_all(root.path()).unwrap_err(), access.write(&outside, b"bad").unwrap_err()] {
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        }
        assert_eq!(access.read(&path).unwrap(), b"inside");
    }).unwrap();
    assert_eq!(FileStore::default().read(outside).unwrap(), b"outside");
}

#[test]
fn directory_entries_remain_owned_after_the_scope_closes() {
    let root = TempDir::new("persistence owned entries");
    fs::write(root.path().join("one"), b"1").unwrap();
    let entries = FileStore::default().with(&[root.path().to_path_buf()], |access| access.read_dir(root.path())).unwrap().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file_name(), "one");
    assert_eq!(entries[0].path(), root.path().join("one"));
    assert!(entries[0].file_type().unwrap().is_file());
}

#[test]
fn creation_and_copy_never_clobber_existing_content_or_consume_the_source() {
    let root = TempDir::new("persistence no clobber");
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::write(&source, b"source bytes").unwrap();
    fs::write(&target, b"kept bytes").unwrap();
    FileStore::default().with(&[root.path().to_path_buf()], |access| {
        assert_eq!(access.create(&target, b"bad").unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(access.publish_checked(&target, b"bad", PublishMode::Create, |_| Ok(()), |error| error).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert!(!access.copy_if_absent(&source, &target).unwrap());
        assert_eq!(access.read(&target).unwrap(), b"kept bytes");
        access.remove_file(&target).unwrap();
        assert!(access.copy_if_absent(&source, &target).unwrap());
        assert_eq!(access.read(&target).unwrap(), b"source bytes");
        assert_eq!(access.read(&source).unwrap(), b"source bytes");
        assert_eq!(access.read_dir(root.path()).unwrap().len(), 2, "no temporary files survive");
    }).unwrap();
}

#[test]
fn existing_copy_target_remains_authoritative_when_source_disappears() {
    let root = TempDir::new("persistence copy existing target");
    let source = root.path().join("missing-source");
    let target = root.path().join("target");
    fs::write(&target, b"already migrated").unwrap();
    FileStore::default().with(&[root.path().to_path_buf()], |access| {
        assert!(!access.copy_if_absent(&source, &target).unwrap());
        assert_eq!(access.read(&target).unwrap(), b"already migrated");
        access.remove_file(&target).unwrap();
        assert_eq!(access.copy_if_absent(&source, &target).unwrap_err().kind(), io::ErrorKind::NotFound);
        assert!(!access.try_exists(&target).unwrap(), "failed copy must remove only its newly created target");
    }).unwrap();
}

#[test]
fn checked_publication_rechecks_and_preserves_old_bytes_on_failure() {
    let root = TempDir::new("persistence checked publication");
    let target = root.path().join("record");
    fs::write(&target, b"old").unwrap();
    let guard = root.path().join("guard");
    fs::write(&guard, b"valid").unwrap();
    FileStore::default().with(&[root.path().to_path_buf()], |access| {
        let mut checks = 0;
        let error = access.publish_checked(&target, b"new", PublishMode::Replace, |view| {
            checks += 1;
            assert_eq!(view.read(&target).unwrap(), b"old");
            if view.read(&guard)? != b"valid" { return Err(io::Error::other("revalidation rejected")); }
            // Controlled invalidation after the first observation, before publication.
            view.write(&guard, b"changed")?;
            Ok(())
        }, |error| error).unwrap_err();
        assert_eq!(error.to_string(), "revalidation rejected");
        assert_eq!(checks, 2);
        assert_eq!(access.read(&target).unwrap(), b"old");
        assert_eq!(access.read_dir(root.path()).unwrap().len(), 2);
        access.publish_checked(&target, b"new", PublishMode::Replace, |_| Ok(()), |error| error).unwrap();
        assert_eq!(access.read(&target).unwrap(), b"new");
    }).unwrap();
}

#[test]
fn create_publication_does_not_overwrite_a_target_created_during_revalidation() {
    let root = TempDir::new("persistence racing create");
    let target = root.path().join("record");
    FileStore::default().with(&[root.path().to_path_buf()], |access| {
        let mut checks = 0;
        let result = access.publish_checked(&target, b"candidate", PublishMode::Create, |view| {
            checks += 1;
            if checks == 2 { view.create(&target, b"winner")?; }
            Ok(())
        }, |error| error);
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(access.read(&target).unwrap(), b"winner");
        assert_eq!(access.read_dir(root.path()).unwrap().len(), 1);
    }).unwrap();
}

#[test]
fn json_update_failure_does_not_publish_mutated_or_unencodable_state() {
    let root = TempDir::new("persistence update failure");
    let target = root.path().join("registry.json");
    fs::write(&target, b"{\"items\":[1]}").unwrap();
    FileStore::default().with(&[root.path().to_path_buf()], |access| {
        let rejected: Result<(Value, ()), String> = access.update_json(&target,
            |view| view.read_json::<Value>(&target),
            |value| { value["items"] = json!([1, 2]); Err("rejected update".into()) },
            |value| serde_json::to_vec(value).map_err(|error| error.to_string()), |error| error.to_string());
        assert_eq!(rejected.unwrap_err(), "rejected update");
        let unencodable: Result<(Value, ()), String> = access.update_json(&target,
            |view| view.read_json::<Value>(&target),
            |value| { value["items"] = json!([1, 2]); Ok(()) },
            |_| Err("encoding rejected".into()), |error| error.to_string());
        assert_eq!(unencodable.unwrap_err(), "encoding rejected");
        assert_eq!(access.read(&target).unwrap(), b"{\"items\":[1]}");
        let result: Result<(Value, usize), String> = access.update_json(&target,
            |view| view.read_json::<Value>(&target),
            |value| { value["items"] = json!([1, 2]); Ok(2) },
            |value| serde_json::to_vec(value).map_err(|error| error.to_string()), |error| error.to_string());
        assert_eq!(result.unwrap(), (json!({"items":[1,2]}), 2));
        assert_eq!(access.read_json::<Value>(&target).unwrap(), json!({"items":[1,2]}));
    }).unwrap();
}

#[test]
fn damaged_jsonl_and_missing_files_are_not_repaired_by_append() {
    let root = TempDir::new("persistence jsonl damage");
    let target = root.path().join("events.jsonl");
    FileStore::default().with(&[root.path().to_path_buf()], |access| {
        assert!(access.append_jsonl::<Value>(&target, &json!({"event":2})).is_err());
        assert!(!access.try_exists(&target).unwrap());
        for bytes in [b"{\"event\":1}".as_slice(), b"not-json\n".as_slice()] {
            access.write(&target, bytes).unwrap();
            assert!(access.append_jsonl::<Value>(&target, &json!({"event":2})).is_err());
            assert_eq!(access.read(&target).unwrap(), bytes);
        }
        access.write(&target, b"{\"event\":1}\n").unwrap();
        access.append_jsonl::<Value>(&target, &json!({"event":2})).unwrap();
        assert_eq!(access.read_jsonl::<Value>(&target).unwrap(), vec![json!({"event":1}), json!({"event":2})]);
    }).unwrap();
}

#[tokio::test]
async fn cancelling_a_waiter_does_not_block_later_waiters_or_lose_the_wakeup() {
    let root = TempDir::new("persistence cancelled waiter");
    let path = root.path().join("record");
    fs::write(&path, b"kept").unwrap();
    let store = FileStore::default();
    let paths = [path.clone()];
    let (release, released) = tokio::sync::oneshot::channel();
    let owner = store.with_async(&paths, move |_| Box::pin(async move { released.await.unwrap(); }));
    tokio::pin!(owner);
    assert_pending(owner.as_mut()).await;
    let mut cancelled = Box::pin(store.with_async(&paths, |_| Box::pin(async {})));
    assert_pending(cancelled.as_mut()).await;
    drop(cancelled);
    let later = store.with_async(&paths, move |access| Box::pin(async move { access.read(path) }));
    tokio::pin!(later);
    assert_pending(later.as_mut()).await;
    release.send(()).unwrap();
    owner.await.unwrap();
    assert_eq!(tokio::time::timeout(Duration::from_secs(10), later).await.unwrap().unwrap().unwrap(), b"kept");
}

#[test]
fn publication_failure_leaves_existing_directory_content_unchanged() {
    let root = TempDir::new("persistence publish failure");
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("original"), b"kept").unwrap();
    FileStore::default().with(&[root.path().to_path_buf()], |access| {
        assert!(access.replace(&target, b"replacement").is_err());
        assert_eq!(access.read(target.join("original")).unwrap(), b"kept");
        assert_eq!(access.read_dir(root.path()).unwrap().len(), 1);
    }).unwrap();
}

#[cfg(windows)]
#[test]
fn failed_file_replacement_preserves_original_bytes_and_cleans_only_owned_temporary() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = TempDir::new("persistence replacement sharing failure");
    let target = root.path().join("original");
    fs::write(&target, b"original bytes").unwrap();
    // Permit reads/writes but deny delete-sharing, so rename cannot replace this file.
    let held = fs::OpenOptions::new().read(true).share_mode(3).open(&target).unwrap();
    FileStore::default().with(&[root.path().to_path_buf()], |access| {
        assert!(access.replace(&target, b"replacement").is_err());
        assert_eq!(access.read(&target).unwrap(), b"original bytes");
        assert_eq!(access.read_dir(root.path()).unwrap().len(), 1);
    }).unwrap();
    drop(held);
}

#[cfg(windows)]
#[tokio::test]
async fn windows_case_and_verbatim_aliases_share_the_same_scope() {
    let root = TempDir::new("persistence windows aliases");
    let path = root.path().join("CaseRecord");
    fs::write(&path, b"value").unwrap();
    let canonical = fs::canonicalize(&path).unwrap();
    let alias = PathBuf::from(canonical.to_string_lossy().to_uppercase());
    let store = FileStore::default();
    let (release, released) = tokio::sync::oneshot::channel();
    let owner_paths = [path];
    let owner = store.with_async(&owner_paths, move |_| Box::pin(async move { released.await.unwrap(); }));
    tokio::pin!(owner);
    assert_pending(owner.as_mut()).await;
    let waiting_paths = [alias.clone()];
    let waiting = store.with_async(&waiting_paths, move |access| Box::pin(async move { access.read(alias) }));
    tokio::pin!(waiting);
    assert_pending(waiting.as_mut()).await;
    release.send(()).unwrap();
    owner.await.unwrap();
    assert_eq!(waiting.await.unwrap().unwrap(), b"value");
}

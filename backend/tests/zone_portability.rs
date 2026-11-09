mod common;
use common::TempDir;
use herdr_bot::zones::Zones;

#[tokio::test]
async fn stable_ids_are_unique_but_directories_are_not() {
    let data = TempDir::new("zone stable identities");
    let zones = Zones::new(data.path().join("zones.json"));
    let directory = data.path().to_str().unwrap();
    zones.register("person/project", directory).await.unwrap();
    zones.register("other/project", directory).await.unwrap();
    assert_eq!(zones.list().await.unwrap().len(), 2);
    assert_eq!(zones.register("person/project", directory).await.unwrap_err().code(), "already-registered");
    assert_eq!(zones.register(" ", directory).await.unwrap_err().code(), "zone-id-required");
    zones.unregister("person/project").await.unwrap();
    assert!(data.path().is_dir());
    assert_eq!(zones.list().await.unwrap()[0].id, "other/project");
}

#[tokio::test]
async fn rebinding_changes_only_existing_id_and_never_creates_directories() {
    let data = TempDir::new("zone rebind");
    let zones = Zones::new(data.path().join("zones.json"));
    let old = data.path().join("old");
    let new = data.path().join("new");
    std::fs::create_dir(&old).unwrap();
    std::fs::create_dir(&new).unwrap();
    zones.register("plain ID without namespace restriction", old.to_str().unwrap()).await.unwrap();
    let before = zones.resolve("plain ID without namespace restriction").await.unwrap();
    zones.rebind("plain ID without namespace restriction", new.to_str().unwrap()).await.unwrap();
    assert_ne!(zones.resolve("plain ID without namespace restriction").await.unwrap(), before);
    let missing = data.path().join("not-created");
    assert!(zones.rebind("plain ID without namespace restriction", missing.to_str().unwrap()).await.is_err());
    assert!(!missing.exists());
    assert_eq!(zones.rebind("unknown", new.to_str().unwrap()).await.unwrap_err().code(), "not-registered");
}

#[tokio::test]
async fn legacy_registry_requires_explicit_migration_and_stays_untouched() {
    let data = TempDir::new("zone explicit migration required");
    let path = data.path().join("zones.json");
    let original = serde_json::json!({"zones":[{"path":data.path()}]}).to_string();
    std::fs::write(&path, &original).unwrap();
    let zones = Zones::new(path.clone());
    let error = zones.list().await.unwrap_err();
    assert_eq!(error.code(), "registry-corrupt");
    assert!(zones.register("explicit/id", data.path().to_str().unwrap()).await.is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

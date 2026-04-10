use super::*;
use crate::db::open_and_migrate;

fn setup() -> rusqlite::Connection {
    open_and_migrate(std::path::Path::new(":memory:")).unwrap()
}

#[test]
fn test_insert_root_returns_correct_fields() {
    let conn = setup();
    let root = insert_root(&conn, "/tmp/test-docs").unwrap();
    assert_eq!(root.path, "/tmp/test-docs");
    assert_eq!(root.label, "test-docs");
    assert!(root.active);
    assert!(root.id > 0);
    assert!(root.created_at > 0);
    assert!(root.last_indexed_at.is_none());
}

#[test]
fn test_find_root_by_path_returns_none_for_missing() {
    let conn = setup();
    let result = find_root_by_path(&conn, "/nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_find_root_by_path_returns_existing() {
    let conn = setup();
    let inserted = insert_root(&conn, "/tmp/docs").unwrap();
    let found = find_root_by_path(&conn, "/tmp/docs").unwrap().unwrap();
    assert_eq!(found.id, inserted.id);
    assert_eq!(found.path, "/tmp/docs");
}

#[test]
fn test_find_root_by_id() {
    let conn = setup();
    let inserted = insert_root(&conn, "/tmp/docs").unwrap();
    let found = find_root_by_id(&conn, inserted.id).unwrap().unwrap();
    assert_eq!(found.path, "/tmp/docs");
    assert!(find_root_by_id(&conn, 9999).unwrap().is_none());
}

#[test]
fn test_update_root_last_indexed() {
    let conn = setup();
    let root = insert_root(&conn, "/tmp/docs").unwrap();
    update_root_last_indexed(&conn, root.id, 1_700_000_000).unwrap();
    let found = find_root_by_id(&conn, root.id).unwrap().unwrap();
    assert_eq!(found.last_indexed_at, Some(1_700_000_000));
}

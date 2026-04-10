use super::*;
use crate::db::{open_and_migrate, unix_now, insert_root, Root};
use rusqlite::params;

fn setup() -> rusqlite::Connection {
    open_and_migrate(std::path::Path::new(":memory:")).unwrap()
}

fn insert_test_root(conn: &rusqlite::Connection) -> Root {
    insert_root(conn, "/tmp/test-root").unwrap()
}

#[test]
fn test_upsert_file_inserts_new_record() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(
        &conn, root.id, "docs/notes.txt", "notes.txt", "txt",
        1024, 1_700_000_000_000_000, "abc123", "", 1, now,
    ).unwrap();
    assert!(id > 0);
    let record = find_file_by_path(&conn, root.id, "docs/notes.txt").unwrap().unwrap();
    assert_eq!(record.fingerprint, "abc123");
    assert_eq!(record.size_bytes, 1024);
}

#[test]
fn test_upsert_file_updates_existing_record() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 100, 1000, "fp1", "", 1, now).unwrap();
    upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 200, 2000, "fp2", "", 2, now).unwrap();
    let record = find_file_by_path(&conn, root.id, "a.txt").unwrap().unwrap();
    assert_eq!(record.fingerprint, "fp2");
    assert_eq!(record.size_bytes, 200);
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_find_file_by_fingerprint() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 100, 1000, "unique_fp", "", 1, now).unwrap();
    let found = find_file_by_fingerprint(&conn, root.id, "unique_fp").unwrap().unwrap();
    assert_eq!(found.rel_path, "a.txt");
    assert!(find_file_by_fingerprint(&conn, root.id, "nonexistent").unwrap().is_none());
}

#[test]
fn test_stamp_index_marker_updates_marker() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 100, 1000, "fp", "", 1, now).unwrap();
    stamp_index_marker(&conn, id, 99, now).unwrap();
    let record = find_file_by_path(&conn, root.id, "a.txt").unwrap().unwrap();
    assert_eq!(record.index_marker, 99);
}

#[test]
fn test_move_file_updates_path() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(&conn, root.id, "old/a.txt", "a.txt", "txt", 100, 1000, "fp", "", 1, now).unwrap();
    move_file(&conn, id, "new/a.txt", "a.txt", 2000, 2, now).unwrap();
    assert!(find_file_by_path(&conn, root.id, "old/a.txt").unwrap().is_none());
    let moved = find_file_by_path(&conn, root.id, "new/a.txt").unwrap().unwrap();
    assert_eq!(moved.index_marker, 2);
}

#[test]
fn test_sweep_deleted_files_soft_deletes_unseen() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    upsert_file_metadata(&conn, root.id, "keep.txt", "keep.txt", "txt", 100, 1000, "fp1", "", 1, now).unwrap();
    upsert_file_metadata(&conn, root.id, "gone.txt", "gone.txt", "txt", 100, 1000, "fp2", "", 0, now).unwrap();
    let deleted = sweep_deleted_files(&conn, root.id, 1, now).unwrap();
    assert_eq!(deleted, 1);
    let active: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(active, 1);
    let deleted_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE root_id = ?1 AND deleted_at IS NOT NULL",
        params![root.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(deleted_count, 1);
}

#[test]
fn test_soft_deleted_file_not_found_by_fingerprint() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(&conn, root.id, "a.txt", "a.txt", "txt", 100, 1000, "fp", "", 0, now).unwrap();
    sweep_deleted_files(&conn, root.id, 1, now).unwrap();
    assert!(find_file_by_fingerprint(&conn, root.id, "fp").unwrap().is_none());
    let deleted: Option<i64> = conn.query_row(
        "SELECT deleted_at FROM files WHERE id = ?1",
        params![id],
        |r| r.get(0),
    ).unwrap();
    assert!(deleted.is_some());
}

#[test]
fn test_update_file_content_persists_fields() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(
        &conn, root.id, "a.txt", "a.txt", "txt",
        100, 1000, "fp", "", 1, now,
    ).unwrap();
    update_file_content(&conn, id, "hello world", 0.95, "en", "ext-v1", now).unwrap();
    let (text, confidence, lang, mv): (String, f64, String, String) = conn.query_row(
        "SELECT extracted_text, confidence, lang_hint, model_version FROM files WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).unwrap();
    assert_eq!(text, "hello world");
    assert!((confidence - 0.95).abs() < 1e-6);
    assert_eq!(lang, "en");
    assert_eq!(mv, "ext-v1");
}

#[test]
fn test_replace_chunks_inserts_all() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(
        &conn, root.id, "a.txt", "a.txt", "txt",
        100, 1000, "fp", "", 1, now,
    ).unwrap();
    let chunks = vec![(0usize, "chunk zero", None), (1, "chunk one", None), (2, "chunk two", None)];
    replace_chunks(&conn, id, &chunks).unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE file_id = ?1",
        params![id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 3);
}

#[test]
fn test_replace_chunks_deletes_stale_on_reindex() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(
        &conn, root.id, "a.txt", "a.txt", "txt",
        100, 1000, "fp", "", 1, now,
    ).unwrap();
    replace_chunks(&conn, id, &[(0, "old chunk a", None), (1, "old chunk b", None)]).unwrap();
    replace_chunks(&conn, id, &[(0, "new single chunk", None)]).unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE file_id = ?1",
        params![id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1);
    let text: String = conn.query_row(
        "SELECT text FROM chunks WHERE file_id = ?1",
        params![id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(text, "new single chunk");
}

#[test]
fn test_chunks_cascade_deleted_with_file() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(
        &conn, root.id, "a.txt", "a.txt", "txt",
        100, 1000, "fp", "", 1, now,
    ).unwrap();
    replace_chunks(&conn, id, &[(0, "chunk", None)]).unwrap();
    conn.execute("DELETE FROM files WHERE id = ?1", params![id]).unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE file_id = ?1",
        params![id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 0, "chunks must be cascade-deleted when the parent file is deleted");
}

#[test]
fn test_find_files_needing_extraction_returns_only_unprocessed() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id_pending = upsert_file_metadata(
        &conn, root.id, "pending.txt", "pending.txt", "txt",
        100, 1000, "fp1", "", 1, now,
    ).unwrap();
    upsert_file_metadata(
        &conn, root.id, "done.txt", "done.txt", "txt",
        100, 1000, "fp2", "ext-v1", 1, now,
    ).unwrap();
    let pending = find_files_needing_extraction(&conn, root.id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, id_pending);
    assert_eq!(pending[0].1, "pending.txt");
    assert_eq!(pending[0].2, "txt");
}

#[test]
fn test_find_files_needing_extraction_excludes_soft_deleted() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(
        &conn, root.id, "a.txt", "a.txt", "txt",
        100, 1000, "fp", "", 0, now,
    ).unwrap();
    sweep_deleted_files(&conn, root.id, 1, now).unwrap();
    let deleted_at: Option<i64> = conn.query_row(
        "SELECT deleted_at FROM files WHERE id = ?1",
        params![id],
        |r| r.get(0),
    ).unwrap();
    assert!(deleted_at.is_some());
    let pending = find_files_needing_extraction(&conn, root.id).unwrap();
    assert!(pending.is_empty());
}

#[test]
fn test_chunks_vec_stores_embedding() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(
        &conn, root.id, "a.txt", "a.txt", "txt", 100, 1000, "fp", "", 1, now,
    ).unwrap();
    let emb: Vec<u8> = (0..768_u32)
        .flat_map(|i| (i as f32 * 0.01).to_le_bytes())
        .collect();
    replace_chunks(&conn, id, &[(0, "hello", Some(emb.as_slice()))]).unwrap();
    let chunk_id: i64 = conn.query_row(
        "SELECT id FROM chunks WHERE file_id = ?1 AND chunk_index = 0",
        params![id],
        |r| r.get(0),
    ).unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks_vec WHERE chunk_id = ?1",
        params![chunk_id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1, "embedding must be stored in chunks_vec");
}

#[test]
fn test_replace_chunks_clears_chunks_vec_on_update() {
    let conn = setup();
    let root = insert_test_root(&conn);
    let now = unix_now();
    let id = upsert_file_metadata(
        &conn, root.id, "b.txt", "b.txt", "txt", 100, 1000, "fp2", "", 1, now,
    ).unwrap();
    let emb: Vec<u8> = vec![0.0_f32; 768].iter().flat_map(|f| f.to_le_bytes()).collect();
    replace_chunks(&conn, id, &[(0, "first", Some(emb.as_slice()))]).unwrap();
    replace_chunks(&conn, id, &[(0, "second", None)]).unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunks_vec cv JOIN chunks c ON c.id = cv.chunk_id WHERE c.file_id = ?1",
        params![id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 0, "old chunks_vec entry must be removed on re-index");
}

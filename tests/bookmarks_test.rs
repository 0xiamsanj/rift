use rift::bookmarks::BookmarkStore;

#[test]
fn test_bookmark_store() {
    let mut store = BookmarkStore::new();

    store.add_session_bookmark("Session BM".to_string(), "/var/log".to_string(), false, None);
    assert_eq!(store.session_bookmarks.len(), 1);

    assert_eq!(store.session_bookmarks[0].label, "Session BM");
    assert_eq!(store.session_bookmarks[0].path, "/var/log");
    assert!(store.session_bookmarks[0].is_session_only);

    let id = store.session_bookmarks[0].id.clone();
    store.remove_session_bookmark(&id);
    assert_eq!(store.session_bookmarks.len(), 0);
}

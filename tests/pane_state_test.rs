use rift::ui::pane_state::{PaneId, PaneState};

#[test]
fn test_navigation_clears_filter_state() {
    let mut pane = PaneState::new(PaneId::Left, false, "/tmp".to_string());
    pane.filter = "foo".to_string();
    pane.filter_active = true;

    pane.navigate_to("/var".to_string());
    assert_eq!(pane.current_path, "/var");
    assert!(pane.filter.is_empty());
    assert!(!pane.filter_active);
}

#[test]
fn test_history_navigation_clears_filter_state() {
    let mut pane = PaneState::new(PaneId::Left, false, "/a".to_string());
    pane.navigate_to("/b".to_string());

    pane.filter = "x".to_string();
    pane.filter_active = true;
    assert!(pane.go_back());
    assert_eq!(pane.current_path, "/a");
    assert!(pane.filter.is_empty());
    assert!(!pane.filter_active);

    pane.filter = "y".to_string();
    pane.filter_active = true;
    assert!(pane.go_forward());
    assert_eq!(pane.current_path, "/b");
    assert!(pane.filter.is_empty());
    assert!(!pane.filter_active);
}

#[test]
fn test_select_last_on_empty_pane() {
    let mut pane = PaneState::new(PaneId::Left, false, "/tmp".to_string());
    assert_eq!(pane.select_last(), None);
    assert_eq!(pane.focused_index, None);
}

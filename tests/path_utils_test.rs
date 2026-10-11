use rift::fs::path_utils::{
    build_breadcrumbs, default_open_command, get_parent_path, is_valid_filename, join_path,
    list_system_roots, normalize_path,
};

#[test]
fn test_windows_drive_normalization() {
    let p = normalize_path("c:\\users\\test\\documents");
    assert_eq!(p, "C:\\users\\test\\documents");

    let p2 = normalize_path("d:/projects//rift");
    assert_eq!(p2, "D:\\projects\\rift");

    let p3 = normalize_path("C:\\");
    assert_eq!(p3, "C:\\");

    let p4 = normalize_path("c:");
    assert_eq!(p4, "C:\\");
}

#[test]
fn test_unc_path_normalization() {
    let unc = normalize_path("\\\\server\\share\\data");
    assert_eq!(unc, "\\\\server\\share\\data");
}

#[test]
fn test_unc_duplicate_separator_collapse() {
    // Forward slashes become backslashes and interior duplicates collapse,
    // while the leading UNC prefix is preserved.
    let unc = normalize_path("//server//share//data");
    assert_eq!(unc, "\\\\server\\share\\data");

    let unc2 = normalize_path("\\\\server\\\\share\\\\data");
    assert_eq!(unc2, "\\\\server\\share\\data");
}

#[test]
fn test_unix_path_normalization() {
    let unix = normalize_path("/var/log//nginx/error.log");
    assert_eq!(unix, "/var/log/nginx/error.log");

    let root = normalize_path("/");
    assert_eq!(root, "/");
}

#[test]
fn test_parent_path_resolution() {
    // Windows remote / local
    assert_eq!(get_parent_path("/var/log", true), Some("/var".to_string()));
    assert_eq!(get_parent_path("/var", true), Some("/".to_string()));
    assert_eq!(get_parent_path("/", true), None);

    // Unix local parent
    assert_eq!(get_parent_path("/var/log", false), Some("/var".to_string()));
    assert_eq!(get_parent_path("/var", false), Some("/".to_string()));
    assert_eq!(get_parent_path("/", false), None);
}

#[test]
fn test_join_path() {
    // Remote root
    assert_eq!(join_path("/home", "user", true), "/home/user");
    assert_eq!(join_path("/", "etc", true), "/etc");

    // Local
    let joined = join_path("/tmp", "test", false);
    assert!(joined.ends_with("test"));
}

#[test]
fn test_breadcrumbs() {
    let crumbs = build_breadcrumbs("/var/log/nginx", true);
    assert_eq!(crumbs.len(), 4);
    assert_eq!(crumbs[0].label, "/");
    assert_eq!(crumbs[0].path, "/");
    assert_eq!(crumbs[1].label, "var");
    assert_eq!(crumbs[1].path, "/var");
    assert_eq!(crumbs[2].label, "log");
    assert_eq!(crumbs[2].path, "/var/log");
    assert_eq!(crumbs[3].label, "nginx");
    assert_eq!(crumbs[3].path, "/var/log/nginx");
    assert!(crumbs[3].is_last);
}

#[test]
fn test_valid_filename() {
    assert!(is_valid_filename("valid_name.txt"));
    assert!(is_valid_filename("my-project-v1.0.tar.gz"));

    // Empty and dot files
    assert!(!is_valid_filename(""));
    assert!(!is_valid_filename("."));
    assert!(!is_valid_filename(".."));

    // Reserved Windows characters
    assert!(!is_valid_filename("file<name>.txt"));
    assert!(!is_valid_filename("test:name.txt"));
    assert!(!is_valid_filename("quote\"test.txt"));
    assert!(!is_valid_filename("pipe|test.txt"));
    assert!(!is_valid_filename("question?test.txt"));
    assert!(!is_valid_filename("star*test.txt"));

    // Reserved DOS device names
    assert!(!is_valid_filename("CON"));
    assert!(!is_valid_filename("con.txt"));
    assert!(!is_valid_filename("PRN"));
    assert!(!is_valid_filename("AUX.log"));
    assert!(!is_valid_filename("NUL"));
    assert!(!is_valid_filename("COM1"));
    assert!(!is_valid_filename("LPT9.txt"));
}

#[test]
fn test_list_system_roots() {
    let roots = list_system_roots();
    assert!(!roots.is_empty());
}

#[test]
fn test_default_open_command() {
    // Never launches anything: only inspects the constructed command.
    let cmd = default_open_command("/tmp/some file with spaces.txt");
    #[cfg(target_os = "windows")]
    assert_eq!(cmd.get_program(), "cmd");
    #[cfg(target_os = "macos")]
    assert_eq!(cmd.get_program(), "open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    assert_eq!(cmd.get_program(), "xdg-open");
    let args: Vec<_> = cmd.get_args().collect();
    assert!(args.iter().any(|a| *a == "/tmp/some file with spaces.txt"));
}

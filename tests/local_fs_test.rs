use std::fs::File;
use std::io::Write;
use rift::fs::local::LocalFs;
use rift::fs::types::{FileKind, SortColumn, SortDirection};

#[test]
fn test_local_fs_listing_and_sorting() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    // Create subdirectories and files
    std::fs::create_dir(root.join("z_dir")).unwrap();
    std::fs::create_dir(root.join("a_dir")).unwrap();

    let mut f1 = File::create(root.join("b_file.txt")).unwrap();
    f1.write_all(b"hello world").unwrap(); // 11 bytes

    let mut f2 = File::create(root.join("a_file.rs")).unwrap();
    f2.write_all(b"fn main() {}").unwrap(); // 12 bytes

    // 1. Sort by Name Ascending (directories first)
    let entries = LocalFs::read_dir(
        root,
        SortColumn::Name,
        SortDirection::Ascending,
        None,
        true,
        true, // show_hidden
    )
    .unwrap();

    assert_eq!(entries.len(), 4);
    // Directories first: a_dir, z_dir
    assert_eq!(entries[0].name, "a_dir");
    assert!(matches!(entries[0].kind, FileKind::Directory));
    assert_eq!(entries[1].name, "z_dir");
    assert!(matches!(entries[1].kind, FileKind::Directory));
    // Then files: a_file.rs, b_file.txt
    assert_eq!(entries[2].name, "a_file.rs");
    assert_eq!(entries[3].name, "b_file.txt");

    // 2. Sort by Size Descending (directories still first)
    let size_entries = LocalFs::read_dir(
        root,
        SortColumn::Size,
        SortDirection::Descending,
        None,
        true,
        true, // show_hidden
    )
    .unwrap();

    // Files: b_file (11 bytes) vs a_file (12 bytes) -> a_file then b_file
    assert_eq!(size_entries[2].name, "a_file.rs");
    assert_eq!(size_entries[3].name, "b_file.txt");
}

#[test]
fn test_local_fs_file_operations() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    // Create directory
    let new_dir_path = root.join("new_folder");
    LocalFs::create_dir(&new_dir_path).unwrap();
    assert!(new_dir_path.exists());

    // Rename directory
    let renamed_dir = root.join("renamed_folder");
    LocalFs::rename_entry(&new_dir_path, &renamed_dir).unwrap();
    assert!(!new_dir_path.exists());
    assert!(renamed_dir.exists());

    // Delete directory
    LocalFs::delete_entry(&renamed_dir).unwrap();
    assert!(!renamed_dir.exists());
}

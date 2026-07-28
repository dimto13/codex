use super::*;
use pretty_assertions::assert_eq;

#[test]
fn updater_path_prefers_the_updater_next_to_aren() {
    let temporary_directory = tempfile::tempdir().expect("create temporary directory");
    let current_executable = temporary_directory.path().join("aren");
    let sibling_updater = temporary_directory.path().join(SIBLING_UPDATER_NAME);
    std::fs::write(&sibling_updater, b"updater").expect("create sibling updater");

    assert_eq!(
        resolve_updater_path(Some(&current_executable)),
        sibling_updater
    );
}

#[test]
fn updater_path_falls_back_to_path_lookup_without_a_sibling() {
    let temporary_directory = tempfile::tempdir().expect("create temporary directory");
    let current_executable = temporary_directory.path().join("aren");

    assert_eq!(
        resolve_updater_path(Some(&current_executable)),
        PathBuf::from(PATH_UPDATER_NAME)
    );
}

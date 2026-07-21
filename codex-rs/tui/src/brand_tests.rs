use super::*;
use pretty_assertions::assert_eq;

#[test]
fn detects_aren_from_executable_name() {
    assert_eq!(
        AppBrand::from_arg0(Some(OsStr::new("/usr/local/bin/aren"))),
        AppBrand::Aren
    );
}

#[test]
fn keeps_codex_brand_for_other_executable_names() {
    assert_eq!(
        AppBrand::from_arg0(Some(OsStr::new("/usr/local/bin/codex"))),
        AppBrand::Codex
    );
    assert_eq!(
        AppBrand::from_arg0(Some(OsStr::new(
            "/usr/local/bin/codex-x86_64-unknown-linux-musl"
        ))),
        AppBrand::Codex
    );
    assert_eq!(AppBrand::from_arg0(None), AppBrand::Codex);
}

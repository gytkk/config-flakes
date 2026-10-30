use std::fs;
use std::path::PathBuf;

pub fn load_fixture(path: &str) -> String {
    let fixture_path = fixture_root().join(path);
    fs::read_to_string(&fixture_path).unwrap_or_else(|error| {
        panic!("failed to read fixture {}: {error}", fixture_path.display())
    })
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("support")
        .join("fixtures")
}

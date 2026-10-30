use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn codex_output_schemas_are_openai_strict_compatible() {
    let schema_paths = output_schema_paths();
    assert!(
        !schema_paths.is_empty(),
        "expected at least one command output schema"
    );

    for schema_path in schema_paths {
        let schema_json = fs::read_to_string(&schema_path).expect("read output schema");
        let schema: serde_json::Value = serde_json::from_str(&schema_json).expect("schema json");
        assert_openai_strict_object_schema(&schema, schema_path.to_string_lossy().as_ref());
    }
}

#[test]
fn feedback_output_schema_exists() {
    let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("feedback-output.schema.json");

    assert!(schema_path.exists(), "missing feedback output schema");
}

fn output_schema_paths() -> Vec<PathBuf> {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let mut paths = fs::read_dir(schema_dir)
        .expect("read schema dir")
        .map(|entry| entry.expect("schema dir entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("-output.schema.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn assert_openai_strict_object_schema(schema: &serde_json::Value, path: &str) {
    if let Some(properties) = schema.get("properties").and_then(|value| value.as_object()) {
        assert_eq!(
            schema
                .get("additionalProperties")
                .and_then(|value| value.as_bool()),
            Some(false),
            "{path} must set additionalProperties=false"
        );
        let required = schema
            .get("required")
            .and_then(|value| value.as_array())
            .unwrap_or_else(|| panic!("{path} must list every property in required"));
        for property in properties.keys() {
            assert!(
                required
                    .iter()
                    .any(|required_field| required_field.as_str() == Some(property.as_str())),
                "{path} must mark property {property} as required"
            );
        }
    }

    if let Some(properties) = schema.get("properties").and_then(|value| value.as_object()) {
        for (name, child) in properties {
            assert_openai_strict_object_schema(child, &format!("{path}.properties.{name}"));
        }
    }
    if let Some(items) = schema.get("items") {
        assert_openai_strict_object_schema(items, &format!("{path}.items"));
    }
}

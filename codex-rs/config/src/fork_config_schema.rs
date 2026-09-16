//! Project the runtime schema into shared-file and fork-overlay schemas.

#![expect(
    clippy::expect_used,
    reason = "schema paths are invariants of the generated runtime schema, verified by schema fixture tests"
)]

use crate::fork_config::FORK_CONFIG_PATHS;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeSet;

fn property_pointer(root: &Value, path: &[&str]) -> String {
    let mut pointer = String::new();
    for key in path {
        loop {
            let node = root.pointer(&pointer).expect("config schema property");
            if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
                pointer = reference
                    .strip_prefix('#')
                    .expect("local schema reference")
                    .to_string();
            } else if node.get("allOf").is_some() {
                pointer.push_str("/allOf/0");
            } else {
                break;
            }
        }
        pointer.push_str(&format!("/properties/{key}"));
    }
    pointer
}

pub(crate) fn shared_schema(mut schema: Value) -> Value {
    for path in FORK_CONFIG_PATHS {
        let pointer = property_pointer(&schema, path);
        let (parent, key) = pointer.rsplit_once('/').expect("property parent");
        schema
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .expect("schema properties")
            .remove(key);
    }
    // Schemars obtains defaults by serializing runtime types, including fork fields.
    for pointer in [
        "/definitions/Tui/properties/keymap/default/global",
        "/definitions/TuiKeymap/properties/global/default",
    ] {
        if let Some(defaults) = schema.pointer_mut(pointer).and_then(Value::as_object_mut) {
            for path in FORK_CONFIG_PATHS {
                defaults.remove(*path.last().expect("nonempty fork path"));
            }
        }
    }
    schema
}

pub(crate) fn overlay_schema(runtime: &Value) -> Value {
    let mut schema = json!({"$schema": "http://json-schema.org/draft-07/schema#", "title": "ForkConfigOverlay", "type": "object", "additionalProperties": false, "properties": {}});
    for path in FORK_CONFIG_PATHS {
        let mut properties = &mut schema["properties"];
        for key in &path[..path.len() - 1] {
            if properties.get(*key).is_none() {
                properties[*key] =
                    json!({"type": "object", "additionalProperties": false, "properties": {}});
            }
            properties = &mut properties[*key]["properties"];
        }
        properties[*path.last().expect("nonempty fork path")] = runtime
            .pointer(&property_pointer(runtime, path))
            .expect("fork property schema")
            .clone();
    }
    let mut references = BTreeSet::new();
    collect_references(&schema, &mut references);
    let mut definitions = serde_json::Map::new();
    while let Some(name) = references.pop_first() {
        if definitions.contains_key(&name) {
            continue;
        }
        let definition = runtime["definitions"][&name].clone();
        collect_references(&definition, &mut references);
        definitions.insert(name, definition);
    }
    schema["definitions"] = Value::Object(definitions);
    schema
}

fn collect_references(value: &Value, references: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(name) = object
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|s| s.strip_prefix("#/definitions/"))
            {
                references.insert(name.to_string());
            }
            for child in object.values() {
                collect_references(child, references);
            }
        }
        Value::Array(array) => {
            for child in array {
                collect_references(child, references);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Client-side mirror of OpenRGB's settings schema filter.
//!
//! The server drops every member that is not in the key's schema or has the wrong type
//! (`SettingsManager::FilterSettingsAgainstSchema`). If *nothing* survives, the filter
//! returns JSON `null` and a MODIFY then calls `update(null)`, which is uncaught: the
//! whole OpenRGB process aborts (reproduced against 1.1; docs/UPSTREAM-FACTS.md).
//! A bare `{}` takes the same path. We therefore validate before sending, and are
//! stricter than the server: a member the server would silently drop is an error here.

use crate::error::{Result, SdkError};
use serde_json::Value;

fn type_matches(value: &Value, ty: &str) -> Option<bool> {
    Some(match ty {
        "bool" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "string" | "language" => value.is_string(),
        "array" => value.is_array(),
        "object" | "profile" => value.is_object(),
        _ => return None,
    })
}

/// `schema` is the reply to `get_settings_schema("")`.
pub(crate) fn validate(schema: &Value, key: &str, value: &Value) -> Result<()> {
    let bad = |m: String| Err(SdkError::InvalidSettings(m));
    let Some(obj) = value.as_object() else {
        return bad(format!("settings for \"{key}\" must be a JSON object"));
    };
    let entry = &schema[key];
    // Unknown keys and `ignore_filter` keys are stored as given (the server still
    // requires an object, which was checked above).
    if entry.is_null() || entry["ignore_filter"] == Value::Bool(true) {
        return Ok(());
    }
    let Some(props) = entry["properties"].as_object() else {
        return bad(format!("\"{key}\" has no settable properties"));
    };
    if obj.is_empty() {
        return bad(format!("empty settings for \"{key}\" would crash OpenRGB"));
    }
    for (name, v) in obj {
        let Some(prop) = props.get(name) else {
            return bad(format!("\"{key}\" has no setting \"{name}\""));
        };
        if let Some(ty) = prop["type"].as_str() {
            // An unknown schema type is rejected by the server too (it returns false).
            if type_matches(v, ty) != Some(true) {
                return bad(format!("\"{key}.{name}\" must be of type {ty}"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({
            "ProfileManager": {"properties": {
                "resume_profile": {"type": "profile"},
                "n": {"type": "integer"},
                "on": {"type": "bool"},
            }},
            "Free": {"ignore_filter": true, "properties": {}},
        })
    }

    #[test]
    fn accepts_valid() {
        validate(
            &schema(),
            "ProfileManager",
            &json!({"resume_profile": {"enabled": true, "name": "x"}, "n": 3}),
        )
        .unwrap();
        validate(&schema(), "Free", &json!({"anything": 1})).unwrap();
        validate(&schema(), "Unregistered", &json!({"a": 1})).unwrap();
    }

    #[test]
    fn rejects_everything_that_would_filter_to_null() {
        for v in [
            json!({}),
            json!("oops"),
            json!(null),
            json!({"resume_profile": "oops"}),
            json!({"nope": 1}),
            json!({"n": 1.5}),
            json!({"on": 1}),
        ] {
            assert!(
                matches!(
                    validate(&schema(), "ProfileManager", &v),
                    Err(SdkError::InvalidSettings(_))
                ),
                "{v} should be rejected"
            );
        }
        // a partly-valid value is also refused rather than silently thinned out
        assert!(validate(&schema(), "ProfileManager", &json!({"n": 1, "nope": 2})).is_err());
        assert!(validate(&schema(), "Free", &json!([1])).is_err());
    }
}

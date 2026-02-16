use serde_json::{json, Value};

/// Merge new summary fields into existing summary JSON.
/// Arrays (options, variables, pros_cons) are merged by label/option.
/// Recommendation is replaced entirely if provided.
pub fn merge_summary(existing_json: Option<&str>, update: &Value) -> String {
    let mut existing: Value = existing_json
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| json!({}));

    // Merge options array (match by label)
    if let Some(new_options) = update.get("options").and_then(|v| v.as_array()) {
        let options = existing
            .get("options")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let merged = merge_array_by_key(options, new_options, "label");
        existing["options"] = Value::Array(merged);
    }

    // Merge variables array (match by label)
    if let Some(new_vars) = update.get("variables").and_then(|v| v.as_array()) {
        let vars = existing
            .get("variables")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let merged = merge_array_by_key(vars, new_vars, "label");
        existing["variables"] = Value::Array(merged);
    }

    // Merge pros_cons array (match by option)
    if let Some(new_pc) = update.get("pros_cons").and_then(|v| v.as_array()) {
        let pc = existing
            .get("pros_cons")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let merged = merge_array_by_key(pc, new_pc, "option");
        existing["pros_cons"] = Value::Array(merged);
    }

    // Recommendation: replace entirely
    if let Some(rec) = update.get("recommendation") {
        existing["recommendation"] = rec.clone();
    }

    serde_json::to_string(&existing).unwrap_or_else(|_| "{}".to_string())
}

/// Merge two arrays of objects by a key field.
/// If an item in `new_items` has the same key value as one in `existing`, it replaces it.
/// Otherwise, the new item is appended.
fn merge_array_by_key(existing: Vec<Value>, new_items: &[Value], key: &str) -> Vec<Value> {
    let mut result = existing;
    for new_item in new_items {
        let new_key = new_item.get(key).and_then(|v| v.as_str());
        if let Some(nk) = new_key {
            if let Some(pos) = result.iter().position(|item| {
                item.get(key).and_then(|v| v.as_str()) == Some(nk)
            }) {
                result[pos] = new_item.clone();
            } else {
                result.push(new_item.clone());
            }
        } else {
            result.push(new_item.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_merge_summary_merges_arrays_by_key_and_replaces_recommendation() {
        let existing = json!({
            "options": [
                {"label": "Stay", "description": "Current role"}
            ],
            "variables": [
                {"label": "Salary", "value": "$100k", "impact": "medium"}
            ],
            "pros_cons": [
                {"option": "Stay", "pros": ["Stable"], "cons": ["Slow growth"]}
            ],
            "recommendation": {
                "choice": "Stay",
                "confidence": "low",
                "reasoning": "Default"
            }
        })
        .to_string();

        let update = json!({
            "options": [
                {"label": "Stay", "description": "Known team"},
                {"label": "Leave", "description": "Higher upside"}
            ],
            "variables": [
                {"label": "Salary", "value": "$130k", "impact": "high"}
            ],
            "pros_cons": [
                {"option": "Leave", "pros": ["Growth"], "cons": ["Risk"]}
            ],
            "recommendation": {
                "choice": "Leave",
                "confidence": "high",
                "reasoning": "Higher long-term upside"
            }
        });

        let merged = merge_summary(Some(&existing), &update);
        let merged_json: Value = serde_json::from_str(&merged).expect("merged summary should be valid json");

        let options = merged_json["options"].as_array().expect("options should be an array");
        assert_eq!(options.len(), 2);
        assert!(
            options
                .iter()
                .any(|o| o["label"] == "Stay" && o["description"] == "Known team")
        );
        assert!(options.iter().any(|o| o["label"] == "Leave"));

        assert_eq!(merged_json["variables"][0]["value"], "$130k");
        assert_eq!(merged_json["pros_cons"].as_array().expect("pros_cons array").len(), 2);
        assert_eq!(merged_json["recommendation"]["choice"], "Leave");
    }

    #[test]
    fn unit_merge_summary_recovers_from_invalid_existing_json() {
        let update = json!({
            "variables": [
                {"label": "Risk tolerance", "value": "Medium"}
            ]
        });

        let merged = merge_summary(Some("not-json"), &update);
        let merged_json: Value = serde_json::from_str(&merged).expect("merged summary should be valid json");

        assert_eq!(merged_json["variables"].as_array().expect("variables array").len(), 1);
        assert_eq!(merged_json["variables"][0]["label"], "Risk tolerance");
    }
}

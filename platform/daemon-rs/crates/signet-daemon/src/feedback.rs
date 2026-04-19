//! Shared parsing helpers for memory feedback payloads.

pub fn parse_scores(raw: Option<&serde_json::Value>) -> Option<Vec<(String, f64)>> {
    let obj = raw?.as_object()?;
    let mut out = Vec::new();
    for (id, score) in obj {
        if id.is_empty() {
            continue;
        }
        let Some(v) = score.as_f64() else {
            continue;
        };
        if !v.is_finite() {
            continue;
        }
        out.push((id.clone(), v.clamp(-1.0, 1.0)));
    }
    if out.is_empty() { None } else { Some(out) }
}

#[cfg(test)]
mod tests {
    use super::parse_scores;
    use serde_json::json;

    #[test]
    fn parse_scores_accepts_and_clamps_scores() {
        let mut parsed = parse_scores(Some(&json!({"a": 2.5, "b": -3.0, "c": 0.5}))).unwrap();
        parsed.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], ("a".to_string(), 1.0));
        assert_eq!(parsed[1], ("b".to_string(), -1.0));
        assert_eq!(parsed[2], ("c".to_string(), 0.5));
    }

    #[test]
    fn parse_scores_rejects_invalid_maps() {
        assert!(parse_scores(None).is_none());
        assert!(parse_scores(Some(&json!(null))).is_none());
        assert!(parse_scores(Some(&json!({"a": "bad"}))).is_none());
        assert!(parse_scores(Some(&json!({}))).is_none());
    }
}

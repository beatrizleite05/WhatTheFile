use crate::errors::AppError;
use serde::Serialize;

const MODEL: &str = "qwen2.5vl:7b";

const MEDIA_TYPES: &[&str] = &["pdf", "docx", "xlsx", "csv", "txt", "md", "png", "jpg", "jpeg"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedQueryPayload {
    pub query_text: String,
    pub media_types: Vec<String>,
    pub root_scope: Vec<String>,
    pub date_from: String,
    pub date_to: String,
    pub min_confidence: f64,
}

/// Ask the LLM to extract structured search intent from a natural language query.
/// Returns a `ParsedQueryPayload` on success; callers fall back to the deterministic
/// parser result on any error.
pub fn parse_query(input: &str, ollama_url: &str) -> Result<ParsedQueryPayload, AppError> {
    let prompt = format!(
        r#"Extract search intent from this query and return ONLY a JSON object with these fields:
- "queryText": the core search terms with filter tokens removed (string)
- "mediaTypes": file types mentioned, from this list only: {media_types} (array of strings)
- "rootScope": folder/location names mentioned after "in" (array of strings, lowercase)
- "dateFrom": start date if mentioned, as YYYY-MM-DD (string, empty if not present)
- "dateTo": end date if mentioned, as YYYY-MM-DD (string, empty if not present)
- "minConfidence": minimum confidence threshold 0.0-1.0 if mentioned (number, 0 otherwise)

Query: {input}

Return only the JSON object, no explanation."#,
        media_types = MEDIA_TYPES.join(", "),
        input = input,
    );

    let body = serde_json::json!({
        "model": MODEL,
        "prompt": prompt,
        "stream": false,
        "format": "json",
        "options": { "temperature": 0 },
    });

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30))
        .build();

    let resp = agent
        .post(&format!("{ollama_url}/api/generate"))
        .send_json(body)
        .map_err(|e| AppError::Llm(format!("query parse request failed: {e}")))?;

    let json: serde_json::Value = resp
        .into_json()
        .map_err(|e| AppError::Llm(format!("query parse response decode failed: {e}")))?;

    let response_text = json["response"]
        .as_str()
        .ok_or_else(|| AppError::Llm("missing 'response' field in generate output".into()))?;

    let parsed = parse_llm_response(response_text)?;
    Ok(sanitize(parsed))
}

fn parse_llm_response(text: &str) -> Result<ParsedQueryPayload, AppError> {
    // Tier 1: direct parse
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        return extract_fields(&v);
    }

    // Tier 2: brace-balanced extraction
    if let Some(start) = text.find('{') {
        let mut depth = 0i32;
        let chars: Vec<char> = text.chars().collect();
        for (i, &c) in chars.iter().enumerate().skip(start) {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let slice: String = chars[start..=i].iter().collect();
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&slice) {
                            return extract_fields(&v);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    // Tier 3: regex field salvage
    let query_text = extract_string_field(text, "queryText").unwrap_or_default();
    Ok(ParsedQueryPayload {
        query_text,
        media_types: vec![],
        root_scope: vec![],
        date_from: String::new(),
        date_to: String::new(),
        min_confidence: 0.0,
    })
}

fn extract_fields(v: &serde_json::Value) -> Result<ParsedQueryPayload, AppError> {
    let query_text = v["queryText"].as_str().unwrap_or("").to_string();
    let media_types = v["mediaTypes"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|s| s.to_lowercase())).collect())
        .unwrap_or_default();
    let root_scope = v["rootScope"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(|s| s.to_lowercase())).collect())
        .unwrap_or_default();
    let date_from = v["dateFrom"].as_str().unwrap_or("").to_string();
    let date_to = v["dateTo"].as_str().unwrap_or("").to_string();
    let min_confidence = v["minConfidence"].as_f64().unwrap_or(0.0);

    Ok(ParsedQueryPayload { query_text, media_types, root_scope, date_from, date_to, min_confidence })
}

fn extract_string_field(text: &str, field: &str) -> Option<String> {
    let pattern = format!(r#""{field}"\s*:\s*"([^"]*)""#);
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(text)?.get(1).map(|m| m.as_str().to_string())
}

/// Remove media_types entries not in the known allowlist and clamp min_confidence to [0,1].
fn sanitize(mut p: ParsedQueryPayload) -> ParsedQueryPayload {
    p.media_types.retain(|mt| MEDIA_TYPES.contains(&mt.as_str()));
    p.min_confidence = p.min_confidence.clamp(0.0, 1.0);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_llm_response_valid_json() {
        let text = r#"{"queryText":"project meeting","mediaTypes":["pdf"],"rootScope":["dropbox"],"dateFrom":"2024-01-01","dateTo":"2024-12-31","minConfidence":0.0}"#;
        let result = parse_llm_response(text).unwrap();
        assert_eq!(result.query_text, "project meeting");
        assert_eq!(result.media_types, vec!["pdf"]);
        assert_eq!(result.root_scope, vec!["dropbox"]);
        assert_eq!(result.date_from, "2024-01-01");
    }

    #[test]
    fn parse_llm_response_extracts_from_prose() {
        let text = r#"Here is the extracted intent: {"queryText":"budget report","mediaTypes":["xlsx"],"rootScope":[],"dateFrom":"","dateTo":"","minConfidence":0.0} Hope that helps!"#;
        let result = parse_llm_response(text).unwrap();
        assert_eq!(result.query_text, "budget report");
        assert_eq!(result.media_types, vec!["xlsx"]);
    }

    #[test]
    fn parse_llm_response_salvages_query_text_on_malformed_json() {
        let text = r#"queryText: "notes from standup""#;
        let result = parse_llm_response(text).unwrap();
        assert_eq!(result.query_text, "notes from standup");
        assert!(result.media_types.is_empty());
    }

    #[test]
    fn sanitize_strips_unknown_media_types() {
        let p = ParsedQueryPayload {
            query_text: "test".into(),
            media_types: vec!["pdf".into(), "pptx".into(), "exe".into()],
            root_scope: vec![],
            date_from: String::new(),
            date_to: String::new(),
            min_confidence: 1.5,
        };
        let result = sanitize(p);
        assert_eq!(result.media_types, vec!["pdf"]);
        assert_eq!(result.min_confidence, 1.0);
    }

    #[test]
    fn parse_query_ollama_unreachable_returns_err() {
        let result = parse_query("find the meeting notes from last quarter", "http://127.0.0.1:19999");
        assert!(matches!(result, Err(AppError::Llm(_))));
    }
}

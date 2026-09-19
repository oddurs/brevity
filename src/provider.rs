//! One request, one summary. Three wire shapes cover every supported backend.

use std::time::Duration;

use serde_json::{json, Value};

use crate::config::{Config, MaxTokensField, Provider};

pub fn summarize(cfg: &Config, text: &str) -> Result<String, String> {
    let system = cfg.render(&cfg.system_prompt, text);
    let user = cfg.render(&cfg.prompt_template, text);

    let raw = match cfg.provider {
        Provider::Anthropic => anthropic(cfg, &system, &user)?,
        Provider::Google => google(cfg, &system, &user)?,
        Provider::OpenAiCompat => openai(cfg, &system, &user, cfg.max_tokens_field)?,
    };

    let cleaned = tidy(&raw);
    if cleaned.is_empty() {
        return Err("the model returned an empty summary".to_string());
    }
    Ok(cleaned)
}

/// Models occasionally wrap the answer in a fence or label it. Strip that.
fn tidy(s: &str) -> String {
    let mut t = s.trim();
    if t.starts_with("```") {
        if let Some(rest) = t.split_once('\n').map(|(_, r)| r) {
            if let Some(end) = rest.rfind("```") {
                t = rest[..end].trim();
            }
        }
    }
    for label in ["Summary:", "SUMMARY:", "Here's the summary:", "Here is the summary:"] {
        if let Some(rest) = t.strip_prefix(label) {
            t = rest.trim_start();
        }
    }
    t.trim().to_string()
}

fn agent(cfg: &Config) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(cfg.timeout))
        .http_status_as_error(false)
        .user_agent(concat!("brevity/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// A failed attempt, and whether trying again could plausibly help.
struct Attempt {
    message: String,
    retryable: bool,
    retry_after: Option<Duration>,
}

/// Rate limits and gateway hiccups are transient and common; behind a hotkey a
/// single 429 otherwise means nothing happens and you get an error chime.
/// Timeouts are deliberately *not* retried - the request already spent the full
/// budget, and doubling a silent wait is worse than failing.
fn post(
    cfg: &Config,
    url: &str,
    headers: &[(&str, String)],
    body: &Value,
) -> Result<Value, String> {
    let mut attempt = 0u32;
    loop {
        match post_once(cfg, url, headers, body) {
            Ok(v) => return Ok(v),
            Err(e) => {
                if !e.retryable || attempt >= cfg.retries {
                    return Err(e.message);
                }
                let wait = e.retry_after.unwrap_or_else(|| backoff(attempt));
                eprintln!(
                    "brevity: {} - retrying in {:.1}s ({}/{})",
                    e.message,
                    wait.as_secs_f32(),
                    attempt + 1,
                    cfg.retries
                );
                std::thread::sleep(wait);
                attempt += 1;
            }
        }
    }
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(500u64 << attempt.min(3))
}

/// 408 and 429 are the server asking us to wait; the 5xx family is its problem,
/// not ours. Everything else - a bad model name, a rejected key - fails
/// identically however many times we ask.
fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504 | 529)
}

fn retry_after_of(value: Option<&str>) -> Option<Duration> {
    let secs: f64 = value?.trim().parse().ok()?;
    if !(0.0..=60.0).contains(&secs) {
        return None;
    }
    Some(Duration::from_secs_f64(secs))
}

fn post_once(
    cfg: &Config,
    url: &str,
    headers: &[(&str, String)],
    body: &Value,
) -> Result<Value, Attempt> {
    let mut req = agent(cfg).post(url).header("content-type", "application/json");
    for (k, v) in headers {
        req = req.header(*k, v.as_str());
    }
    for (k, v) in &cfg.extra_headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let mut resp = req.send_json(body).map_err(|e| {
        let timed_out = matches!(e, ureq::Error::Timeout(_));
        Attempt {
            message: net_error(cfg, e),
            // A connection that failed fast is worth another try; one that ate
            // the whole timeout budget is not.
            retryable: !timed_out,
            retry_after: None,
        }
    })?;

    let status = resp.status().as_u16();
    let retry_after =
        retry_after_of(resp.headers().get("retry-after").and_then(|v| v.to_str().ok()));

    let text = resp.body_mut().read_to_string().map_err(|e| Attempt {
        message: format!("could not read the response: {e}"),
        retryable: true,
        retry_after: None,
    })?;

    let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !(200..300).contains(&status) {
        return Err(Attempt {
            message: format!(
                "{} returned HTTP {status}: {}",
                cfg.provider_name,
                api_error(&parsed, &text)
            ),
            retryable: is_retryable_status(status),
            retry_after,
        });
    }
    if parsed.is_null() {
        return Err(Attempt {
            message: format!("{} returned a non-JSON response: {}", cfg.provider_name, clip(&text)),
            retryable: false,
            retry_after: None,
        });
    }
    Ok(parsed)
}

fn net_error(cfg: &Config, e: ureq::Error) -> String {
    let base = e.to_string();
    if base.contains("timed out") || base.contains("Timeout") {
        return format!(
            "{} timed out after {}s (raise BREVITY_TIMEOUT_SECS)",
            cfg.provider_name,
            cfg.timeout.as_secs()
        );
    }
    format!("could not reach {} at {}: {base}", cfg.provider_name, cfg.base_url)
}

fn api_error(parsed: &Value, raw: &str) -> String {
    for path in [
        parsed.pointer("/error/message"),
        parsed.pointer("/error/0/message"),
        parsed.pointer("/message"),
        parsed.pointer("/detail"),
    ] {
        if let Some(Value::String(s)) = path {
            return s.clone();
        }
    }
    clip(raw)
}

fn clip(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() > 400 {
        let cut: String = s.chars().take(400).collect();
        format!("{cut}...")
    } else {
        s.to_string()
    }
}

fn key(cfg: &Config) -> Result<String, String> {
    cfg.api_key.clone().ok_or_else(|| "no API key configured".to_string())
}

// ---------------------------------------------------------------- Anthropic

fn anthropic(cfg: &Config, system: &str, user: &str) -> Result<String, String> {
    let mut body = json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "system": system,
        "messages": [{"role": "user", "content": user}],
    });
    if let Some(effort) = &cfg.effort {
        body["output_config"] = json!({ "effort": effort });
    }
    // Temperature is rejected by the Claude 5 family; only send it if asked for.
    if let Some(t) = cfg.temperature {
        body["temperature"] = json!(t);
    }

    let url = format!("{}/v1/messages", cfg.base_url);
    let headers = [("x-api-key", key(cfg)?), ("anthropic-version", "2023-06-01".to_string())];
    let v = post(cfg, &url, &headers, &body)?;

    if v["stop_reason"] == json!("refusal") {
        return Err("the model declined to summarize this text".to_string());
    }

    let out: String = v["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == json!("text"))
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();

    if out.trim().is_empty() && v["stop_reason"] == json!("max_tokens") {
        return Err(
            "hit the output limit before writing anything (raise BREVITY_MAX_TOKENS)".to_string()
        );
    }
    Ok(out)
}

// ------------------------------------------------------- OpenAI-compatible

fn openai(cfg: &Config, system: &str, user: &str, field: MaxTokensField) -> Result<String, String> {
    let field_name = match field {
        MaxTokensField::MaxTokens => "max_tokens",
        MaxTokensField::MaxCompletionTokens => "max_completion_tokens",
        // Newer OpenAI models reject `max_tokens`; everyone else still expects it.
        MaxTokensField::Auto if cfg.provider_name == "openai" => "max_completion_tokens",
        MaxTokensField::Auto => "max_tokens",
    };

    let mut body = json!({
        "model": cfg.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        "stream": false,
    });
    body[field_name] = json!(cfg.max_tokens);
    if let Some(t) = cfg.temperature {
        body["temperature"] = json!(t);
    }

    let url = format!("{}/chat/completions", cfg.base_url);
    let mut headers: Vec<(&str, String)> = Vec::new();
    if let Some(k) = &cfg.api_key {
        headers.push(("authorization", format!("Bearer {k}")));
    }

    let v = match post(cfg, &url, &headers, &body) {
        Ok(v) => v,
        Err(e) if field == MaxTokensField::Auto && mentions_token_field(&e) => {
            // Some endpoints want the other spelling; take the hint and retry once.
            let other = if field_name == "max_tokens" {
                MaxTokensField::MaxCompletionTokens
            } else {
                MaxTokensField::MaxTokens
            };
            return openai(cfg, system, user, other);
        }
        Err(e) => return Err(e),
    };

    let msg = &v["choices"][0]["message"];
    let out = match &msg["content"] {
        Value::String(s) => s.clone(),
        // A few gateways return content as an array of parts.
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p["text"].as_str().or_else(|| p.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    };

    if out.trim().is_empty() && v["choices"][0]["finish_reason"] == json!("length") {
        return Err(
            "hit the output limit before writing anything (raise BREVITY_MAX_TOKENS)".to_string()
        );
    }
    Ok(out)
}

fn mentions_token_field(err: &str) -> bool {
    let e = err.to_lowercase();
    (e.contains("max_tokens") || e.contains("max_completion_tokens"))
        && (e.contains("unsupported")
            || e.contains("not supported")
            || e.contains("unrecognized")
            || e.contains("instead")
            || e.contains("unknown"))
}

// ------------------------------------------------------------------ Google

fn google(cfg: &Config, system: &str, user: &str) -> Result<String, String> {
    let mut gen = json!({ "maxOutputTokens": cfg.max_tokens });
    if let Some(t) = cfg.temperature {
        gen["temperature"] = json!(t);
    }
    let body = json!({
        "systemInstruction": {"parts": [{"text": system}]},
        "contents": [{"role": "user", "parts": [{"text": user}]}],
        "generationConfig": gen,
    });

    let url = format!("{}/v1beta/models/{}:generateContent", cfg.base_url, cfg.model);
    let headers = [("x-goog-api-key", key(cfg)?)];
    let v = post(cfg, &url, &headers, &body)?;

    let out: String = v["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
        .unwrap_or_default();

    if out.trim().is_empty() {
        if let Some(reason) = v["candidates"][0]["finishReason"].as_str() {
            if reason != "STOP" {
                return Err(format!("Gemini stopped early: {reason}"));
            }
        }
        if let Some(reason) = v["promptFeedback"]["blockReason"].as_str() {
            return Err(format!("Gemini blocked the request: {reason}"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::tidy;

    #[test]
    fn plain_text_survives_untouched() {
        assert_eq!(tidy("  A tight summary.  "), "A tight summary.");
    }

    #[test]
    fn code_fences_are_stripped() {
        assert_eq!(tidy("```\nthe summary\n```"), "the summary");
        assert_eq!(tidy("```markdown\nthe summary\n```"), "the summary");
    }

    #[test]
    fn a_leading_label_is_stripped() {
        assert_eq!(tidy("Summary: the point"), "the point");
        assert_eq!(tidy("Here is the summary: the point"), "the point");
    }

    #[test]
    fn a_fenced_and_labelled_answer_loses_both() {
        assert_eq!(tidy("```\nSummary: the point\n```"), "the point");
    }

    #[test]
    fn a_summary_that_is_itself_about_code_keeps_its_inner_fence() {
        let out = tidy("```\nRun `cargo build`:\n\n```sh\ncargo build\n```\n```");
        assert!(out.contains("cargo build"), "{out}");
    }

    #[test]
    fn only_transient_statuses_are_retried() {
        for s in [408, 429, 500, 502, 503, 504, 529] {
            assert!(super::is_retryable_status(s), "{s} should be retried");
        }
        // A bad key, a bad model, a malformed request: asking again cannot help.
        for s in [400, 401, 403, 404, 413, 422, 501] {
            assert!(!super::is_retryable_status(s), "{s} should not be retried");
        }
    }

    #[test]
    fn backoff_grows_and_then_stops_growing() {
        let d: Vec<u128> = (0..6).map(|i| super::backoff(i).as_millis()).collect();
        assert_eq!(d[0], 500);
        assert_eq!(d[1], 1000);
        assert!(d.windows(2).all(|w| w[1] >= w[0]), "backoff went backwards: {d:?}");
        assert!(d.iter().all(|&ms| ms <= 4000), "backoff unbounded: {d:?}");
    }

    #[test]
    fn a_servers_retry_after_is_honoured_when_sane() {
        assert_eq!(super::retry_after_of(Some("2")).unwrap().as_secs(), 2);
        assert_eq!(super::retry_after_of(Some(" 1.5 ")).unwrap().as_millis(), 1500);
    }

    #[test]
    fn an_absurd_retry_after_is_ignored_rather_than_obeyed() {
        // Some gateways send an HTTP-date or a very long wait; neither is worth
        // blocking a hotkey on.
        assert!(super::retry_after_of(Some("3600")).is_none());
        assert!(super::retry_after_of(Some("Wed, 21 Oct 2026 07:28:00 GMT")).is_none());
        assert!(super::retry_after_of(Some("-5")).is_none());
        assert!(super::retry_after_of(None).is_none());
    }

    #[test]
    fn the_word_summary_mid_sentence_is_left_alone() {
        assert_eq!(tidy("The summary: it shipped."), "The summary: it shipped.");
    }
}

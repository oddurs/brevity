//! A guard against the obvious mistake: pressing the hotkey while a credential
//! is on the clipboard sends it to a third party and writes it to disk. There
//! is no way to un-send that, so the cheap check is worth having.
//!
//! Deliberately high-precision. A false positive blocks work and teaches people
//! to set the override; a missed secret costs a rotation. The rules below only
//! match shapes that are credentials and essentially nothing else.

/// A token that starts with `prefix` and continues with at least `min_tail`
/// characters from the usual key alphabet.
struct Rule {
    name: &'static str,
    prefix: &'static str,
    min_tail: usize,
    /// Require the tail to be uppercase, as AWS key ids are.
    upper: bool,
}

const RULES: &[Rule] = &[
    Rule { name: "an Anthropic API key", prefix: "sk-ant-", min_tail: 20, upper: false },
    Rule { name: "an OpenRouter API key", prefix: "sk-or-v1-", min_tail: 20, upper: false },
    Rule { name: "an OpenAI API key", prefix: "sk-proj-", min_tail: 20, upper: false },
    Rule { name: "an OpenAI API key", prefix: "sk-svcacct-", min_tail: 20, upper: false },
    Rule { name: "a GitHub token", prefix: "ghp_", min_tail: 30, upper: false },
    Rule { name: "a GitHub token", prefix: "gho_", min_tail: 30, upper: false },
    Rule { name: "a GitHub token", prefix: "ghs_", min_tail: 30, upper: false },
    Rule { name: "a GitHub token", prefix: "github_pat_", min_tail: 30, upper: false },
    Rule { name: "a GitLab token", prefix: "glpat-", min_tail: 18, upper: false },
    Rule { name: "a Slack token", prefix: "xoxb-", min_tail: 20, upper: false },
    Rule { name: "a Slack token", prefix: "xoxp-", min_tail: 20, upper: false },
    Rule { name: "a Google API key", prefix: "AIza", min_tail: 33, upper: false },
    Rule { name: "an AWS access key id", prefix: "AKIA", min_tail: 16, upper: true },
    Rule { name: "an AWS access key id", prefix: "ASIA", min_tail: 16, upper: true },
    Rule { name: "a Stripe secret key", prefix: "sk_live_", min_tail: 20, upper: false },
    Rule { name: "a Stripe secret key", prefix: "rk_live_", min_tail: 20, upper: false },
    Rule { name: "an npm token", prefix: "npm_", min_tail: 30, upper: false },
    Rule { name: "a Hugging Face token", prefix: "hf_", min_tail: 30, upper: false },
];

fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Name the kind of credential the text appears to contain, if any.
pub fn detect(text: &str) -> Option<&'static str> {
    // A private key announces itself; no length heuristics needed.
    if text.contains("-----BEGIN") && text.contains("PRIVATE KEY-----") {
        return Some("a private key");
    }

    for rule in RULES {
        let mut rest = text;
        while let Some(at) = rest.find(rule.prefix) {
            let tail = &rest[at + rule.prefix.len()..];
            let run: String = tail.chars().take_while(|c| is_key_char(*c)).collect();
            let long_enough = run.len() >= rule.min_tail;
            let case_ok = !rule.upper || run.chars().all(|c| !c.is_ascii_lowercase());
            if long_enough && case_ok {
                return Some(rule.name);
            }
            // Keep looking; an earlier near-miss should not mask a real one.
            rest = &rest[at + rule.prefix.len()..];
        }
    }

    if looks_like_jwt(text) {
        return Some("a JSON Web Token");
    }

    None
}

/// `header.payload.signature`, base64url, starting with the encoded `{"`.
fn looks_like_jwt(text: &str) -> bool {
    let mut rest = text;
    while let Some(at) = rest.find("eyJ") {
        let candidate: String = rest[at..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            .collect();
        let parts: Vec<&str> = candidate.split('.').collect();
        if parts.len() == 3 && parts.iter().all(|p| p.len() >= 8) {
            return true;
        }
        rest = &rest[at + 3..];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::detect;

    #[test]
    fn it_catches_the_keys_this_tool_is_configured_with() {
        // Shapes only - none of these are real keys.
        assert_eq!(detect("sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAA"), Some("an Anthropic API key"));
        assert_eq!(
            detect("sk-or-v1-0123456789abcdef0123456789abcdef"),
            Some("an OpenRouter API key")
        );
    }

    #[test]
    fn it_catches_the_usual_developer_credentials() {
        assert!(detect("ghp_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").is_some());
        assert!(detect("AKIAIOSFODNN7EXAMPLE").is_some());
        assert!(detect("AIzaSyA0000000000000000000000000000000").is_some());
        assert!(detect("glpat-AAAAAAAAAAAAAAAAAAAA").is_some());
        assert_eq!(
            detect("-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END"),
            Some("a private key")
        );
    }

    #[test]
    fn a_credential_buried_in_a_longer_document_still_counts() {
        let doc = "Here are the deploy notes for Q3.\n\nexport TOKEN=ghp_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n\nSee the runbook.";
        assert!(detect(doc).is_some(), "a key inside a document is still a key");
    }

    #[test]
    fn jwts_are_recognized() {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(detect(jwt), Some("a JSON Web Token"));
    }

    #[test]
    fn ordinary_prose_is_not_a_credential() {
        for text in [
            "The board met on 12 March and agreed three things.",
            "Run `brew install skhd` and then grant Accessibility.",
            "My password is hunter2 but that is not a token shape.",
            "https://example.com/some-very-long-url-path-that-goes-on-and-on-and-on",
            "A discussion of sk-ant- prefixes in the abstract.",
            "AKIA is an AWS prefix, mentioned here without a key.",
        ] {
            assert_eq!(detect(text), None, "false positive on: {text}");
        }
    }

    #[test]
    fn a_short_lookalike_is_not_enough() {
        assert_eq!(detect("sk-ant-short"), None);
        assert_eq!(detect("ghp_tooshort"), None);
    }

    #[test]
    fn aws_ids_are_uppercase_so_lowercase_lookalikes_are_ignored() {
        assert_eq!(detect("AKIAiosfodnn7examplexx"), None, "lowercase tail is not an AWS id");
        assert!(detect("AKIAIOSFODNN7EXAMPLE").is_some());
    }

    #[test]
    fn a_near_miss_does_not_mask_a_real_one_later() {
        let text = "sk-ant-short and then sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAA";
        assert!(detect(text).is_some(), "stopped scanning after the first near-miss");
    }

    #[test]
    fn an_empty_or_tiny_clipboard_is_fine() {
        assert_eq!(detect(""), None);
        assert_eq!(detect("hi"), None);
    }
}

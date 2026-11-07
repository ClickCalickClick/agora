//! Quiet home page (SPEC §4.2): keeps Reddit's chat client and ad and tracker
//! scripts from running background traffic in the hidden view. Reddit's own
//! reCAPTCHA stays allowed (posting rule 2, SPEC §4.5). Agora's calls are
//! unaffected. WebKit's filter syntax has no `|`, hence one rule per host.

const BLOCKED_HOSTS: &[&str] = &[
    r"matrix\.redditspace\.com",
    r"alb\.reddit\.com",
    r"doubleclick\.net",
    r"googlesyndication\.com",
    r"googletagmanager\.com",
    r"google-analytics\.com",
    r"googleadservices\.com",
];

/// The content-blocker rules, as WebKit's filter store compiles them.
pub fn rules() -> String {
    let rules: Vec<serde_json::Value> = BLOCKED_HOSTS
        .iter()
        .map(|host| {
            serde_json::json!({
                "trigger": { "url-filter": format!("^[a-z]+://([^/]*\\.)?{host}/") },
                "action": { "type": "block" },
            })
        })
        .collect();
    serde_json::Value::Array(rules).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recaptcha_is_never_blocked() {
        let rules = rules();
        assert!(rules.contains("matrix"));
        assert!(!rules.contains("recaptcha") && !rules.contains("gstatic"));
    }
}

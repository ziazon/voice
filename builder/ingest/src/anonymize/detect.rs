use regex::Regex;
use std::sync::LazyLock;
static DETECTORS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    [
("email",r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+"),
("phone",r"(?:\+\d{1,3}[\s.-]?)?\(?\b\d{3}\)?[\s.-]\d{3}[\s.-]\d{4}\b"),
("phone",r"\+\d{1,3}(?:[\s.-]?\d{2,4}){2,5}\b"),
("long number",r"\b\d{7,}\b|\b\d{3}-\d{2}-\d{4}\b"),
("secret",r#"\bsk-[A-Za-z0-9_-]{20,}|\bsk_(?:live|test)_[A-Za-z0-9]{16,}|\bgh[pousr]_[A-Za-z0-9]{20,}|\bgithub_pat_[A-Za-z0-9_]{20,}|\bxox[abprs]-[A-Za-z0-9-]{10,}|\bxapp-[A-Za-z0-9-]{10,}|\bAKIA[0-9A-Z]{16}\b|\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}|\bAIza[0-9A-Za-z_-]{30,}|\bhf_[A-Za-z0-9]{30,}|\bglpat-[A-Za-z0-9_-]{20,}|\bnpm_[A-Za-z0-9]{30,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|(?i:\b(?:passw(?:or)?d|secret|token|api[_-]?key|access[_-]?key)\b["']?\s*[:=]\s*["']?[A-Za-z0-9/+_.-]{8,})|(?i:\b[a-z][a-z0-9+.-]*://[^/\s:@]+:[^/\s@]+@)"#),
("home folder",r"~[A-Za-z][\w.-]*/"),
("private host",r"(?i)\b[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*\.(?:local|lan|internal|intranet|corp|home\.arpa|ts\.net)\b"),
("private address",r"\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|100\.(?:6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.\d{1,3}\.\d{1,3}|169\.254\.\d{1,3}\.\d{1,3})\b"),
("ticket id",r"\b([A-Z][A-Z0-9]{1,9})-\d{1,6}\b"),
].into_iter().map(|(kind,pattern)|(kind,Regex::new(pattern).expect("fixed detector regex"))).collect()
});
const STANDARDS: &[&str] = &[
    "AES", "ARM", "CSS", "CVE", "CWE", "ECMA", "ES", "FIPS", "GPT", "HTML", "HTTP", "IEC", "IEEE",
    "IPV", "ISO", "MD", "NIST", "OWASP", "PEP", "PKCS", "RFC", "RSA", "SHA", "SP", "SSL", "TLS",
    "UCS", "UTF", "WCAG", "X86",
];
pub(super) fn find(
    text: &str,
) -> impl Iterator<Item = (std::ops::Range<usize>, &'static str)> + '_ {
    DETECTORS.iter().flat_map(move |(kind, re)| {
        re.captures_iter(text).filter_map(move |cap| {
            let m = cap.get(0).expect("whole match");
            if *kind == "email" {
                let domain = m
                    .as_str()
                    .rsplit('@')
                    .next()
                    .unwrap_or_default()
                    .to_lowercase();
                if ["example.com", "example.org", "example.net"].contains(&domain.as_str())
                    || [".example", ".test", ".invalid", ".localhost"]
                        .iter()
                        .any(|suffix| domain.ends_with(suffix))
                {
                    return None;
                }
            }
            if *kind == "private host" {
                let after = &text[m.end()..];
                if after.starts_with(['-', '_'])
                    || after
                        .strip_prefix('.')
                        .and_then(|s| s.chars().next())
                        .is_some_and(|c| c.is_alphanumeric())
                {
                    return None;
                }
            }
            if *kind == "ticket id" && STANDARDS.contains(&cap[1].as_ref()) {
                return None;
            }
            Some((m.range(), *kind))
        })
    })
}

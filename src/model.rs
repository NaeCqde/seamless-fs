//! What is sent between the servers and the relay (the format has not changed), and the check of what comes in.

use crate::config::web_url;
use serde::{Deserialize, Serialize};

/// One file. `origin` and `url` are set only on the files of other servers.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub origin: Option<String>,
    /// The directory, written `/a/b/` (both ends with a slash; the root is `/`).
    pub parent: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub url: Option<String>,
    pub name: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Payload {
    pub origin: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub files: Option<Vec<Entry>>,
}

impl Entry {
    /// A file of this server, in the directory `parent` (`/a/b/`).
    pub fn local(parent: String, name: String, size: u64) -> Self {
        Self {
            origin: None,
            parent,
            url: None,
            name,
            size,
        }
    }

    /// The entry as this server accepts it, or `None` if it must not be shown. What the other servers send ends up in links
    /// and in HTML, so a name with a slash, a `..` directory or a link that is not http(s) is dropped here.
    pub fn sanitized(mut self) -> Option<Self> {
        if self.name.is_empty()
            || self.name == "."
            || self.name == ".."
            || self.name.contains('/')
            || self.name.chars().any(char::is_control)
        {
            return None;
        }

        self.parent = normalize_dir(&self.parent)?;

        if let Some(origin) = &self.origin {
            self.origin = Some(
                web_url("origin", origin)
                    .ok()?
                    .to_string()
                    .trim_end_matches('/')
                    .to_owned(),
            );
        }
        if let Some(url) = &self.url {
            self.url = Some(web_url("url", url).ok()?.to_string());
        }
        Some(self)
    }
}

/// `a/b` -> `/a/b/`. A directory with `..`, a backslash or a control character is refused.
pub fn normalize_dir(dir: &str) -> Option<String> {
    let mut out = String::from("/");
    for segment in dir.split('/').filter(|s| !s.is_empty() && *s != ".") {
        if segment == ".." || segment.contains('\\') || segment.chars().any(char::is_control) {
            return None;
        }
        out.push_str(segment);
        out.push('/');
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(parent: &str, name: &str) -> Entry {
        Entry::local(parent.into(), name.into(), 1)
    }

    #[test]
    fn directories_are_written_with_a_slash_at_both_ends() {
        assert_eq!(normalize_dir("").as_deref(), Some("/"));
        assert_eq!(normalize_dir("/").as_deref(), Some("/"));
        assert_eq!(normalize_dir("a/b").as_deref(), Some("/a/b/"));
        assert_eq!(normalize_dir("/a//b/./").as_deref(), Some("/a/b/"));
        assert_eq!(normalize_dir("/a/../b"), None);
        assert_eq!(normalize_dir("/a\\b"), None);
    }

    #[test]
    fn odd_names_are_dropped() {
        assert!(entry("/", "ok.txt").sanitized().is_some());
        assert!(entry("/", "").sanitized().is_none());
        assert!(entry("/", "..").sanitized().is_none());
        assert!(entry("/", "a/b").sanitized().is_none());
        assert!(entry("/", "a\nb").sanitized().is_none());
        assert!(entry("/../x", "a").sanitized().is_none());
    }

    #[test]
    fn only_web_links_are_kept() {
        let mut e = entry("/", "a");
        e.url = Some("javascript:alert(1)".into());
        assert!(e.sanitized().is_none());

        let mut e = entry("/", "a");
        e.origin = Some("data:text/html,x".into());
        assert!(e.sanitized().is_none());

        let mut e = entry("/", "a");
        e.origin = Some("https://other.example.com/".into());
        e.url = Some("https://cdn.example.com/a b.txt".into());
        let e = e.sanitized().unwrap();
        assert_eq!(e.origin.as_deref(), Some("https://other.example.com"));
        assert_eq!(e.url.as_deref(), Some("https://cdn.example.com/a%20b.txt"));
    }

    #[test]
    fn the_wire_format_is_unchanged() {
        let json = serde_json::to_string(&entry("/a/", "b.txt")).unwrap();
        assert_eq!(json, r#"{"parent":"/a/","name":"b.txt","size":1}"#);
        let back: Payload = serde_json::from_str(
            r#"{"origin":"http://x","files":[{"parent":"/","name":"a","size":2}]}"#,
        )
        .unwrap();
        assert_eq!(back.files.unwrap()[0].size, 2);
        let none: Payload = serde_json::from_str(r#"{"origin":"http://x"}"#).unwrap();
        assert!(none.files.is_none());
    }
}

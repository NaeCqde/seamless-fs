//! The page that lists a directory: the files here, and the files of the other servers (marked with a link sign).

use crate::{
    confine::InsideRoot,
    model::{normalize_dir, Entry},
    registry::Registry,
};
use humansize::{format_size, DECIMAL};
use ntex::{
    http::Response,
    web::{HttpRequest, WebResponse},
};
use ntex_files::Directory;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use std::{cmp::Ordering, collections::HashMap, fmt::Write, io};

/// Everything but letters, digits and `- _ . ~ /` is written as %XX in a link (a quote or a `#` in a name must not end the link).
const PATH: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~')
    .remove(b'/');

/// Text for the inside of an element or of a quoted attribute.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

pub fn encode_path(path: &str) -> String {
    utf8_percent_encode(path, PATH).to_string()
}

/// One line of the page.
#[derive(Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub href: String,
    pub size: Option<u64>,
    pub external: bool,
}

/// The page for the directory `dir` (`/a/b/`). The text is escaped here and nowhere else.
pub fn render(dir: &str, folders: &[Row], files: &[Row]) -> String {
    let title = escape(&format!("Index of {dir}"));
    let mut items = String::new();
    for row in folders.iter().chain(files) {
        let class = if row.external {
            " class=\"external\""
        } else {
            ""
        };
        let size = row
            .size
            .map(|s| format!(" {}", format_size(s, DECIMAL)))
            .unwrap_or_default();
        let slash = if row.size.is_none() { "/" } else { "" };
        let _ = write!(
            items,
            "<li{class}><a href=\"{}\">{}{slash}</a>{size}</li>",
            escape(&row.href),
            escape(&row.name)
        );
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>.external::marker {{ content: '🔗 ' }}</style></head>\
         <body><h1>{title}</h1><p>🔗 = File from another server</p><ul>{items}</ul></body></html>"
    )
}

/// The local entries come first, then by name; so a listing does not move when the other servers change.
fn order(a: &Row, b: &Row) -> Ordering {
    a.external
        .cmp(&b.external)
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.href.cmp(&b.href))
}

fn files_order(a: &Row, b: &Row) -> Ordering {
    a.name
        .cmp(&b.name)
        .then_with(|| a.external.cmp(&b.external))
        .then_with(|| a.href.cmp(&b.href))
}

/// Where a file of another server is. `url` is the address when the sender gave one, otherwise it is below the server of the file.
fn external_href(file: &Entry) -> Option<String> {
    if let Some(url) = &file.url {
        return Some(url.clone());
    }
    let origin = file.origin.as_deref()?;
    Some(format!(
        "{origin}{}",
        encode_path(&format!("{}{}", file.parent, file.name))
    ))
}

/// The renderer for `ntex_files::Files::files_listing_renderer`.
pub fn directory_listing(
    registry: &Registry,
    root: &InsideRoot,
    dir: &Directory,
    req: &HttpRequest,
) -> Result<WebResponse, io::Error> {
    let decoded = percent_decode_str(req.path()).decode_utf8_lossy();
    let base = normalize_dir(&decoded).unwrap_or_else(|| "/".to_owned());

    // this server
    let mut folders: HashMap<String, Row> = HashMap::new();
    let mut files: Vec<Row> = Vec::new();
    for entry in dir.path.read_dir()? {
        if !dir.is_visible(&entry) {
            continue;
        }
        let Ok(entry) = entry else { continue };
        // a link that leaves the root is not offered (and the file server would refuse it)
        if entry.file_type().is_ok_and(|t| t.is_symlink()) && !root.contains(&entry.path()) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();

        if meta.is_dir() {
            let href = encode_path(&format!("{base}{name}/"));
            folders.insert(
                name.clone(),
                Row {
                    name,
                    href,
                    size: None,
                    external: false,
                },
            );
        } else {
            let href = encode_path(&format!("{base}{name}"));
            files.push(Row {
                name,
                href,
                size: Some(meta.len()),
                external: false,
            });
        }
    }

    // the other servers (a folder that exists here too stays a folder of this server)
    let (remote_dirs, remote_files) = registry.list(&base);
    for folder in remote_dirs {
        folders.entry(folder.name.clone()).or_insert_with(|| Row {
            href: format!(
                "{}{}",
                folder.origin,
                encode_path(&format!("{base}{}/", folder.name))
            ),
            name: folder.name,
            size: None,
            external: true,
        });
    }
    for file in remote_files {
        if let Some(href) = external_href(&file) {
            files.push(Row {
                name: file.name,
                href,
                size: Some(file.size),
                external: true,
            });
        }
    }

    let mut folders: Vec<Row> = folders.into_values().collect();
    folders.sort_by(order);
    files.sort_by(files_order);

    Ok(WebResponse::new(
        Response::Ok()
            .content_type("text/html; charset=utf-8")
            // the page has no script and loads nothing
            .header(
                "content-security-policy",
                "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'",
            )
            .body(render(&base, &folders, &files)),
        req.to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, href: &str, size: Option<u64>, external: bool) -> Row {
        Row {
            name: name.into(),
            href: href.into(),
            size,
            external,
        }
    }

    #[test]
    fn links_survive_odd_characters() {
        assert_eq!(encode_path("/a b/c#d?e\"f.txt"), "/a%20b/c%23d%3Fe%22f.txt");
        assert_eq!(
            encode_path("/日本/a-b_c.d~e"),
            "/%E6%97%A5%E6%9C%AC/a-b_c.d~e"
        );
    }

    #[test]
    fn names_and_links_are_escaped_in_the_page() {
        let html = render(
            "/<b>/",
            &[],
            &[row(
                "<script>x</script>",
                "/\"onmouseover=\"alert(1)",
                Some(1500),
                false,
            )],
        );
        assert!(!html.contains("<script>"));
        assert!(!html.contains("\"onmouseover"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("1.50 kB"));
        assert!(html.contains("Index of /&lt;b&gt;/"));
    }

    #[test]
    fn external_rows_are_marked_and_folders_end_with_a_slash() {
        let html = render(
            "/",
            &[row("sub", "/sub/", None, false)],
            &[row("f", "https://x/f", Some(1), true)],
        );
        assert!(html.contains("<li><a href=\"/sub/\">sub/</a></li>"));
        assert!(html.contains("<li class=\"external\"><a href=\"https://x/f\">f</a> 1 B</li>"));
    }

    #[test]
    fn links_to_files_of_other_servers() {
        let mut file = Entry::local("/a b/".into(), "c.txt".into(), 1);
        file.origin = Some("https://x.example.com".into());
        assert_eq!(
            external_href(&file).unwrap(),
            "https://x.example.com/a%20b/c.txt"
        );

        file.url = Some("https://cdn.example.com/c.txt".into());
        assert_eq!(
            external_href(&file).unwrap(),
            "https://cdn.example.com/c.txt"
        );

        assert!(Entry::local("/".into(), "c".into(), 1).origin.is_none());
        assert!(external_href(&Entry::local("/".into(), "c".into(), 1)).is_none());
    }

    #[test]
    fn local_rows_come_first() {
        let mut rows = [
            row("a", "/z", Some(1), true),
            row("b", "/b", Some(1), false),
        ];
        rows.sort_by(order);
        assert_eq!(rows[0].name, "b");
        let mut rows = [
            row("a", "https://x/a", Some(1), true),
            row("a", "/a", Some(1), false),
        ];
        rows.sort_by(files_order);
        assert!(!rows[0].external);
    }
}

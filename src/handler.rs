use crate::data::{File_, Payload, STATE};

use humansize::{format_size, DECIMAL};
use ntex::{
    http::Response,
    web::{types, HttpRequest, WebResponse},
};
use ntex_files::Directory;
use percent_encoding::{percent_decode_str, utf8_percent_encode, CONTROLS};
use reqwest::{header, Client};
use serde_json::json;
use std::{collections::HashMap, fmt::Write, io, path::Path};
use v_htmlescape::escape as escape_html_entity;
use walkdir::WalkDir;

#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;

// show file url as relative to static path
macro_rules! encode_file_url {
    ($path:ident) => {
        utf8_percent_encode(&$path, CONTROLS).to_string()
    };
}
macro_rules! decode_file_url {
    ($path:ident) => {
        percent_decode_str(&$path).decode_utf8_lossy().to_string()
    };
}

macro_rules! file_name {
    ($entry:ident) => {
        $entry.file_name().to_string_lossy().to_string()
    };
}

fn or_insert(s: &str, start: bool, end: bool) -> String {
    let mut s = s.to_owned();
    if start || end {
        if start && !s.starts_with("/") {
            s = format!("/{}", s);
        }
        if end && !s.ends_with("/") {
            s = format!("{}/", s);
        }
    }
    return s;
}

pub fn directory_listing(dir: &Directory, req: &HttpRequest) -> Result<WebResponse, io::Error> {
    let base = {
        let path = req.path();
        &or_insert(&decode_file_url!(path), false, true)
    };
    let index_of = format!("Index of {}", base);
    let mut body = String::new();

    let mut folders: HashMap<String, File_> = HashMap::new();
    let mut files: Vec<File_> = Vec::new();

    for entry in dir.path.read_dir()? {
        if dir.is_visible(&entry) {
            let entry = entry.unwrap();
            let p = match entry.path().strip_prefix(&dir.path) {
                Ok(p) if cfg!(windows) => Path::new(&base)
                    .join(p)
                    .to_string_lossy()
                    .replace('\\', "/"),
                Ok(p) => Path::new(&base).join(p).to_string_lossy().into_owned(),
                Err(_) => continue,
            };

            // if file is a directory, add '/' to the end of the name
            if let Ok(metadata) = entry.metadata() {
                let name = file_name!(entry);

                if let Some(parent) = &p.strip_suffix(&name) {
                    if metadata.is_dir() {
                        folders.insert(
                            name.to_owned(),
                            File_ {
                                origin: None,
                                parent: parent.to_string(),
                                url: None,
                                name: name.to_owned(),
                                size: 0,
                            },
                        );
                        //println!("name:{}, parent:{}, p:{}", &name, parent, p);
                    } else {
                        files.push(File_ {
                            origin: None,
                            parent: parent.to_string(),
                            url: None,
                            name: file_name!(entry),

                            #[cfg(not(target_os = "windows"))]
                            size: metadata.size(),
                            #[cfg(target_os = "windows")]
                            size: metadata.file_size(),
                        });
                    }
                }
            } else {
                continue;
            }
        }
    }

    {
        let origins = STATE.origins.read().expect("failed to read state");

        for (org, org_files) in origins.iter() {
            for org_f in org_files {
                let parent = Path::new(&org_f.parent);
                //println!("{}, {}", parent.to_string_lossy(), base);
                if Path::new(&base) == parent {
                    let mut f = org_f.to_owned();

                    if let Some(url) = f.url {
                        f.url = Some(decode_file_url!(url));
                    }
                    if f.origin.is_none() {
                        f.origin = Some(org.to_string());
                    }

                    files.push(f);
                } else if let Ok(stripped) = parent.strip_prefix(base) {
                    if let Some(next) = stripped.iter().next() {
                        let next: String = next.to_string_lossy().to_string();
                        /*println!(
                            "parent:{} base:{}, next:{}",
                            parent.to_string_lossy(),
                            or_insert(base, false, false),
                            next
                        );*/
                        //println!("{}", base);

                        //let splited: Vec<&str> = next.splitn(2, "/").collect();
                        //if splited.len() == 2 {

                        folders.entry(next.to_owned()).or_insert(File_ {
                            origin: Some(org.to_string()),
                            parent: base.to_owned(),
                            url: None,
                            name: next,
                            size: 0,
                        });
                        // }
                    }
                }
            }
        }
    }

    let mut folders = folders.into_values().collect::<Vec<File_>>();
    folders.sort_by(|a, b| {
        // origin None を先に
        match (&a.origin, &b.origin) {
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name), // name で昇順
        }
    });

    for folder in folders {
        if let Some(org) = &folder.origin {
            let path = format!("{}{}{}/", &org, &folder.parent, &folder.name);

            let _ = write!(
                body,
                "<li class=\"external\"><a href=\"{}\">{}/</a></li>",
                encode_file_url!(path),
                escape_html_entity(&folder.name),
            );
        } else {
            let path = format!("{}{}/", &folder.parent, &folder.name);

            let _ = write!(
                body,
                "<li><a href=\"{}\">{}/</a></li>",
                encode_file_url!(path),
                escape_html_entity(&folder.name),
            );
        }
        /*println!(
            "origin: {:?}, parent: {}, name: {}",
            folder.origin, folder.parent, folder.name
        );*/
    }

    //println!("\n\n\n");

    files.sort_by(|a, b| {
        let ord_name = a.name.cmp(&b.name);
        if ord_name != std::cmp::Ordering::Equal {
            return ord_name;
        }

        // name が同じなら origin: None を先に、Some(_) は中で昇順
        match (&a.origin, &b.origin) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(a_origin), Some(b_origin)) => a_origin.cmp(b_origin),
        }
    });

    for file in files {
        if let Some(url) = file.url {
            let _ = write!(
                body,
                "<li class=\"external\"><a href=\"{}\">{}</a> {}</li>",
                encode_file_url!(url),
                escape_html_entity(&file.name),
                format_size(file.size, DECIMAL),
            );

            /*println!(
                "url: {:?}, parent: {}, name: {}, size: {}",
                url,
                file.parent,
                file.name,
                format_size(file.size, DECIMAL),
            );*/
        } else if let Some(org) = file.origin {
            let url = {
                let u = format!("{}{}{}", &org, &file.parent, &file.name);
                encode_file_url!(u)
            };

            let _ = write!(
                body,
                "<li class=\"external\"><a href=\"{}\">{}</a> {}</li>",
                url,
                escape_html_entity(&file.name),
                format_size(file.size, DECIMAL),
            );

            /*println!(
                "origin: {:?}, parent: {}, name: {}, size: {}",
                org,
                file.parent,
                file.name,
                format_size(file.size, DECIMAL),
            );*/
        } else {
            let path = format!("{}{}", &file.parent, &file.name);

            let _ = write!(
                body,
                "<li><a href=\"{}\">{}</a> {}</li>",
                encode_file_url!(path),
                escape_html_entity(&file.name),
                format_size(file.size, DECIMAL),
            );
        }
        /*println!(
            "origin: None, parent: {}, name: {}, size: {}",
            file.parent,
            file.name,
            format_size(file.size, DECIMAL),
        );*/
    }

    let html = format!(
        "<html><head>\
        <title>{}</title>\
        <style>.external::marker {{ content: '🔗 ' }}</style>\
        </head>\
        <body>\
        <h1>{}</h1>\
        <p>🔗 = File from another server</p>\
        <ul>{}</ul>\
        </body></html>",
        index_of, index_of, body
    );
    Ok(WebResponse::new(
        Response::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        req.to_owned(),
    ))
}

// receive new or update remote server
pub async fn handle_insert_origin(payload: types::Json<Payload>) -> Response {
    if let Some(files) = &payload.files {
        let mut origins = STATE.origins.write().expect("failed to lock origins");
        origins.insert(payload.origin.to_owned(), files.to_vec());
    }

    return Response::Ok().finish();
}
// receive down remote server
pub async fn handle_delete_origin(payload: types::Json<Payload>) -> Response {
    {
        let mut origins = STATE.origins.write().expect("failed to lock origins");
        origins.remove(&payload.origin);
    }

    return Response::Ok().finish();
}

// receive local file updates
pub async fn handle_update_notify(first: bool) {
    let mut files: Vec<File_> = Vec::new();

    for entry in WalkDir::new(".").into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".") {
                continue;
            }

            let mut parent = entry
                .path()
                .parent()
                .get_or_insert(Path::new("/"))
                .to_string_lossy()
                .to_string();

            if parent.starts_with(".") {
                parent = parent.replacen(".", "", 1);
                //println!("{}", parent);
                parent = or_insert(&parent, true, true);
            }

            files.push(File_ {
                origin: None,
                parent,
                url: None,
                name,

                #[cfg(not(target_os = "windows"))]
                size: entry.metadata().expect("failed to get metadata").size(),
                #[cfg(target_os = "windows")]
                size: entry
                    .metadata()
                    .expect("failed to get metadata")
                    .file_size(),
            })
        }
    }

    send_files(first, files).await;
}

// send files to relay
pub async fn send_files(first: bool, files: Vec<File_>) {
    let client = Client::default();
    let payload = &json!({
        "origin": STATE.origin,
        "files":files,
    });

    let mut url = STATE.relay_url.to_owned();
    if first {
        url.push_str("?first=true")
    }

    let _ = client
        .put(url)
        .header(header::AUTHORIZATION, &STATE.token)
        .header(header::CONTENT_TYPE, "application/json")
        .json(payload)
        .send()
        .await
        .map_err(|e| format!("request failure: {}", e));
}

// notify down this server to relay
pub async fn delete_origin() {
    let client = Client::default();
    let payload = &json!({
        "origin": STATE.origin
    });

    let _ = client
        .delete(&STATE.relay_url)
        .header(header::AUTHORIZATION, &STATE.token)
        .header(header::CONTENT_TYPE, "application/json")
        .json(payload)
        .send()
        .await
        .map_err(|e| format!("request failure: {}", e));
}

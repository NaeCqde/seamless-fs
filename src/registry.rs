//! The files of the other servers, as the relay told us. Kept per directory, so listing one directory does not read the files
//! of the whole tree.

use crate::config::web_url;
use crate::model::{normalize_dir, Entry};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::RwLock,
};

/// What one relay can make us keep. The body of a request is limited as well (see `main.rs`).
pub const MAX_ORIGINS: usize = 256;
pub const MAX_ENTRIES_PER_ORIGIN: usize = 20_000;

#[derive(Debug, PartialEq)]
pub enum InsertError {
    BadOrigin,
    TooManyOrigins,
    TooManyEntries,
}

/// A folder of another server.
#[derive(Debug, PartialEq)]
pub struct RemoteDir {
    pub origin: String,
    pub name: String,
}

#[derive(Default)]
struct Dir {
    files: Vec<Entry>,
    subdirs: BTreeSet<String>,
}

#[derive(Default)]
struct Remote {
    dirs: HashMap<String, Dir>,
}

#[derive(Default)]
pub struct Registry {
    // ordered, so that the same folder of two servers is always taken from the same one
    remotes: RwLock<BTreeMap<String, Remote>>,
}

/// The keys of the registry are written the same way whatever the relay sends.
fn key(origin: &str) -> Result<String, InsertError> {
    web_url("origin", origin)
        .map(|u| u.to_string().trim_end_matches('/').to_owned())
        .map_err(|_| InsertError::BadOrigin)
}

impl Registry {
    /// Replaces the files of `origin`. Files that fail the check are left out; the number of those that were kept is returned.
    pub fn replace(&self, origin: &str, files: Vec<Entry>) -> Result<usize, InsertError> {
        let origin = key(origin)?;
        if files.len() > MAX_ENTRIES_PER_ORIGIN {
            return Err(InsertError::TooManyEntries);
        }

        let mut remote = Remote::default();
        let mut kept = 0;
        for entry in files.into_iter().filter_map(Entry::sanitized) {
            // the folders on the way down, so that each of them can be listed
            let mut dir = String::from("/");
            for segment in entry.parent.split('/').filter(|s| !s.is_empty()) {
                remote
                    .dirs
                    .entry(dir.clone())
                    .or_default()
                    .subdirs
                    .insert(segment.to_owned());
                dir.push_str(segment);
                dir.push('/');
            }
            remote.dirs.entry(dir).or_default().files.push(entry);
            kept += 1;
        }

        let mut remotes = self.write();
        if !remotes.contains_key(&origin) && remotes.len() >= MAX_ORIGINS {
            return Err(InsertError::TooManyOrigins);
        }
        remotes.insert(origin, remote);
        Ok(kept)
    }

    pub fn remove(&self, origin: &str) -> Result<(), InsertError> {
        let origin = key(origin)?;
        self.write().remove(&origin);
        Ok(())
    }

    /// The folders and files that other servers have in `dir` (`/a/b/`). A file carries its own `origin` (the server it is on).
    pub fn list(&self, dir: &str) -> (Vec<RemoteDir>, Vec<Entry>) {
        let Some(dir) = normalize_dir(dir) else {
            return (Vec::new(), Vec::new());
        };
        let remotes = self.read();

        let mut folders = Vec::new();
        let mut seen = HashSet::new();
        let mut files = Vec::new();
        for (origin, remote) in remotes.iter() {
            let Some(found) = remote.dirs.get(&dir) else {
                continue;
            };
            for name in &found.subdirs {
                if seen.insert(name.clone()) {
                    folders.push(RemoteDir {
                        origin: origin.clone(),
                        name: name.clone(),
                    });
                }
            }
            for file in &found.files {
                let mut file = file.clone();
                file.origin.get_or_insert_with(|| origin.clone());
                files.push(file);
            }
        }
        (folders, files)
    }

    // a panic while the lock was held leaves the data usable: the listing goes on
    fn read(&self) -> std::sync::RwLockReadGuard<'_, BTreeMap<String, Remote>> {
        self.remotes.read().unwrap_or_else(|e| e.into_inner())
    }
    fn write(&self) -> std::sync::RwLockWriteGuard<'_, BTreeMap<String, Remote>> {
        self.remotes.write().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(parent: &str, name: &str) -> Entry {
        Entry::local(parent.into(), name.into(), 10)
    }

    #[test]
    fn folders_and_files_are_listed_per_directory() {
        let registry = Registry::default();
        registry
            .replace(
                "http://a.example.com/",
                vec![file("/", "top.txt"), file("/x/y/", "deep.txt")],
            )
            .unwrap();

        let (folders, files) = registry.list("/");
        assert_eq!(
            folders,
            vec![RemoteDir {
                origin: "http://a.example.com".into(),
                name: "x".into()
            }]
        );
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].origin.as_deref(), Some("http://a.example.com"));

        let (folders, files) = registry.list("/x/");
        assert_eq!(folders[0].name, "y");
        assert!(files.is_empty());

        let (folders, files) = registry.list("/x/y/");
        assert!(folders.is_empty());
        assert_eq!(files[0].name, "deep.txt");
    }

    #[test]
    fn the_same_folder_on_two_servers_comes_from_the_first_server_by_name() {
        let registry = Registry::default();
        registry
            .replace("http://b.example.com", vec![file("/shared/", "b.txt")])
            .unwrap();
        registry
            .replace("http://a.example.com", vec![file("/shared/", "a.txt")])
            .unwrap();

        let (folders, _) = registry.list("/");
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].origin, "http://a.example.com");
        assert_eq!(registry.list("/shared/").1.len(), 2);
    }

    #[test]
    fn replacing_and_removing() {
        let registry = Registry::default();
        registry
            .replace("http://a.example.com", vec![file("/", "one")])
            .unwrap();
        registry
            .replace("http://a.example.com/", vec![file("/", "two")])
            .unwrap();
        let names: Vec<_> = registry.list("/").1.into_iter().map(|f| f.name).collect();
        assert_eq!(names, ["two"]);

        registry.remove("http://a.example.com").unwrap();
        assert!(registry.list("/").1.is_empty());
    }

    #[test]
    fn bad_input_is_refused_or_dropped() {
        let registry = Registry::default();
        assert_eq!(
            registry.replace("javascript:x", vec![]),
            Err(InsertError::BadOrigin)
        );

        let kept = registry
            .replace(
                "http://a.example.com",
                vec![file("/", "ok"), file("/../x/", "bad"), file("/", "a/b")],
            )
            .unwrap();
        assert_eq!(kept, 1);

        let many = (0..=MAX_ENTRIES_PER_ORIGIN)
            .map(|i| file("/", &i.to_string()))
            .collect();
        assert_eq!(
            registry.replace("http://a.example.com", many),
            Err(InsertError::TooManyEntries)
        );
    }

    #[test]
    fn the_number_of_servers_is_limited() {
        let registry = Registry::default();
        for i in 0..MAX_ORIGINS {
            registry
                .replace(&format!("http://s{i}.example.com"), vec![])
                .unwrap();
        }
        assert_eq!(
            registry.replace("http://extra.example.com", vec![]),
            Err(InsertError::TooManyOrigins)
        );
        // a server that is known can still be updated
        assert!(registry
            .replace("http://s0.example.com", vec![file("/", "x")])
            .is_ok());
    }
}

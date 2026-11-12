//! The files of this server, as they are told to the relay.

use crate::{confine::InsideRoot, model::Entry};
use std::path::{Component, Path};
use walkdir::{DirEntry, WalkDir};

pub fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|n| n.starts_with('.'))
}

/// Every file under the root, except hidden ones (a name that starts with a dot, and everything in a hidden folder) and links that
/// lead out of the root. The file server refuses all of these too, so nothing is advertised that cannot be fetched.
pub fn scan(root: &InsideRoot) -> Vec<Entry> {
    let mut files = Vec::new();

    let walker = WalkDir::new(root.root())
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_hidden(e.file_name()));

    for entry in walker.filter_map(Result::ok) {
        let Some(size) = file_size(root, &entry) else {
            continue;
        };
        let Ok(relative) = entry.path().strip_prefix(root.root()) else {
            continue;
        };

        let mut parent = String::from("/");
        if let Some(dir) = relative.parent() {
            for part in dir.components() {
                if let Component::Normal(part) = part {
                    parent.push_str(&part.to_string_lossy());
                    parent.push('/');
                }
            }
        }
        files.push(Entry::local(
            parent,
            entry.file_name().to_string_lossy().into_owned(),
            size,
        ));
    }
    files
}

fn file_size(root: &InsideRoot, entry: &DirEntry) -> Option<u64> {
    let kind = entry.file_type();
    if kind.is_file() {
        return entry.metadata().ok().map(|m| m.len());
    }
    // a link to a file inside the root is a file like any other
    if kind.is_symlink() && root.contains(entry.path()) {
        let meta = std::fs::metadata(entry.path()).ok()?;
        return meta.is_file().then(|| meta.len());
    }
    None
}

/// Is this path hidden, seen from the root? (Used to ignore changes that cannot matter.)
pub fn is_hidden_path(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .any(|c| matches!(c, Component::Normal(n) if is_hidden(n)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn files_are_found_with_their_directory_and_size() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("top.txt"), "12345").unwrap();
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/b/deep.txt"), "12").unwrap();
        let root = InsideRoot::new(dir.path()).unwrap();

        let mut files = scan(&root);
        files.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(files.len(), 2);
        assert_eq!(
            (
                files[0].parent.as_str(),
                files[0].name.as_str(),
                files[0].size
            ),
            ("/a/b/", "deep.txt", 2)
        );
        assert_eq!(
            (
                files[1].parent.as_str(),
                files[1].name.as_str(),
                files[1].size
            ),
            ("/", "top.txt", 5)
        );
    }

    #[test]
    fn hidden_files_and_hidden_folders_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".env"), "TOKEN=x").unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git/config"), "x").unwrap();
        fs::write(dir.path().join("shown.txt"), "x").unwrap();
        let root = InsideRoot::new(dir.path()).unwrap();

        let files = scan(&root);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "shown.txt");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_root_is_left_out() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "x").unwrap();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("real.txt"), "x").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            dir.path().join("leak.txt"),
        )
        .unwrap();
        std::os::unix::fs::symlink(dir.path().join("real.txt"), dir.path().join("alias.txt"))
            .unwrap();
        let root = InsideRoot::new(dir.path()).unwrap();

        let mut names: Vec<_> = scan(&root).into_iter().map(|f| f.name).collect();
        names.sort();
        assert_eq!(names, ["alias.txt", "real.txt"]);
    }

    #[test]
    fn hidden_paths() {
        let root = Path::new("/srv");
        assert!(is_hidden_path(root, Path::new("/srv/.git/index")));
        assert!(is_hidden_path(root, Path::new("/srv/a/.env")));
        assert!(!is_hidden_path(root, Path::new("/srv/a/b.txt")));
    }
}

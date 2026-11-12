//! Keeps everything that is served inside the root directory.
//!
//! `ntex-files` rejects `..` and names that start with a dot, but it follows symbolic links without checking where they lead:
//! a link inside the root that points to `/etc` would expose that directory. [`InsideRoot`] closes this. As a guard it makes a
//! request that would leave the root fall through to the 404 of the router, and the scan and the listing use
//! [`InsideRoot::contains`] to leave such links out.

use ntex::{http::RequestHead, web::guard::Guard};
use percent_encoding::percent_decode_str;
use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct InsideRoot {
    root: PathBuf,
}

impl InsideRoot {
    /// `root` is resolved once (symbolic links included), so later checks compare against the real directory.
    pub fn new(root: &Path) -> io::Result<Self> {
        Ok(Self {
            root: root.canonicalize()?,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Does `path` (a symbolic link is followed) end up inside the root? A path that does not exist is not inside.
    pub fn contains(&self, path: &Path) -> bool {
        path.canonicalize()
            .map(|real| real.starts_with(&self.root))
            .unwrap_or(false)
    }

    /// Would a request for this URL path be served from inside the root? A path that does not exist is allowed here: the file
    /// server answers it with a 404 itself.
    pub fn allows(&self, url_path: &str) -> bool {
        let Ok(decoded) = percent_decode_str(url_path).decode_utf8() else {
            return false;
        };

        let mut path = self.root.clone();
        for segment in decoded.split('/') {
            match segment {
                "" | "." => {}
                ".." => return false,
                // a separator or a drive letter inside a segment would make `join` leave the root on Windows
                s if s.contains(['\\', ':', '\0']) => return false,
                s => path.push(s),
            }
        }

        match path.canonicalize() {
            Ok(real) => real.starts_with(&self.root),
            Err(_) => true,
        }
    }
}

impl Guard for InsideRoot {
    fn check(&self, request: &RequestHead) -> bool {
        self.allows(request.uri.path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn plain_paths_are_allowed() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        fs::write(dir.path().join("a/b.txt"), "x").unwrap();
        let root = InsideRoot::new(dir.path()).unwrap();

        assert!(root.allows("/"));
        assert!(root.allows("/a/"));
        assert!(root.allows("/a/b.txt"));
        assert!(root.allows("/missing.txt")); // the file server answers 404
    }

    #[test]
    fn parent_segments_and_odd_characters_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = InsideRoot::new(dir.path()).unwrap();

        assert!(!root.allows("/../etc/passwd"));
        assert!(!root.allows("/a/%2e%2e/b"));
        assert!(!root.allows("/a%5Cb"));
        assert!(!root.allows("/C:/Windows"));
        assert!(!root.allows("/%ff")); // not UTF-8
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leaves_the_root_is_refused() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "x").unwrap();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("inside.txt"), "x").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("inside.txt"), dir.path().join("in")).unwrap();
        let root = InsideRoot::new(dir.path()).unwrap();

        assert!(!root.allows("/out/secret.txt"));
        assert!(!root.contains(&dir.path().join("out")));
        assert!(root.allows("/in"));
        assert!(root.contains(&dir.path().join("in")));
    }
}

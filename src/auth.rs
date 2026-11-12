//! The token that the relay (and nobody else) uses to register and remove servers.

use ntex::{http::header::AUTHORIZATION, http::RequestHead, web::guard::Guard};
use subtle::ConstantTimeEq;

/// Lets a request through only if its `Authorization` header is exactly the token. The comparison takes the same time wherever the
/// two differ, so the token cannot be found out one character at a time.
#[derive(Clone)]
pub struct TokenGuard {
    token: Vec<u8>,
}

impl TokenGuard {
    pub fn new(token: &str) -> Self {
        Self {
            token: token.as_bytes().to_vec(),
        }
    }

    fn accepts(&self, header: &[u8]) -> bool {
        self.token.ct_eq(header).into()
    }
}

impl Guard for TokenGuard {
    fn check(&self, request: &RequestHead) -> bool {
        request
            .headers
            .get(AUTHORIZATION)
            .is_some_and(|value| self.accepts(value.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_token_is_accepted() {
        let guard = TokenGuard::new("correct horse battery");
        assert!(guard.accepts(b"correct horse battery"));
        assert!(!guard.accepts(b"correct horse batterz"));
        assert!(!guard.accepts(b"correct horse"));
        assert!(!guard.accepts(b"correct horse battery "));
        assert!(!guard.accepts(b""));
    }
}

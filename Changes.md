# Changes

## Unreleased

- Security
  - Files outside the served directory are no longer reachable through a symbolic link.
  - The token is compared in constant time; a request without it gets 401.
  - What other servers send is checked: only http(s) links, no odd names; names and links in the page are escaped properly (a quote or `#` in a file name could break out of a link).
  - Hidden files and hidden folders are no longer advertised to the relay; the page has a Content-Security-Policy, and `nosniff` is sent.
  - Limits on the registration (body size, files per server, number of servers).
  - An empty `TOKEN` or a non-http(s) `ORIGIN` / `RELAY_URL` refuses to start; a short token or a plain-http relay is warned about.
- Behaviour
  - SIGTERM stops the server like Ctrl-C (and withdraws it from the relay).
  - The same folder on several servers is always taken from the same one.
  - Requests to the relay have timeouts and their failures are logged.
- Code is split into modules and has unit tests; the registry is kept per directory.
- The macOS x86_64 build is gone.

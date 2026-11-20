# Seamless File Server (`sfs`)

English | [日本語](README.md)

Serves the files of the current directory over HTTP, and lists the files of the other `sfs` servers through the relay as well.

```bash
cp example.env .env    # fill in HOST / PORT / ORIGIN / RELAY_URL / TOKEN
sfs
```

Make `TOKEN` a random string of at least 16 characters (shared with the relay). For the specification and what to watch out for, see [docs/SPEC.md](docs/SPEC.md).

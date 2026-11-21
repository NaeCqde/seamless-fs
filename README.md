# Seamless File Server (`sfs`)

[English](README.en.md) | 日本語

カレントディレクトリのファイルを HTTP で配り、リレー経由で他の `sfs` サーバーのファイルも一覧に並べる。

```bash
sfs token              # TOKEN= (このサーバーの .env に) と SHA256= (リレーの SERVERS に)
sfs keygen             # リレーの鍵の組(最初に一度だけ。公開鍵は全サーバーとリレーに)
cp example.env .env    # HOST / PORT / ORIGIN / RELAY_URL / RELAY_PUBLIC_KEY / TOKEN を書く
sfs
```

`TOKEN` はサーバーごとに別々。リレー([seamless-fs-relay](https://github.com/NaeCqde/seamless-fs-relay))にはそのSHA-256だけを置く。仕様と注意は [docs/SPEC.md](docs/SPEC.md)。

## 設定ファイルの場所

`.env` は、まずカレントディレクトリから探します。カレントディレクトリにあれば、それだけを読みます。なければ、実行ファイルがあるディレクトリのファイルを読みます。

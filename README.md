# Seamless File Server (`sfs`)

[English](README.en.md) | 日本語

カレントディレクトリのファイルを HTTP で配り、リレー経由で他の `sfs` サーバーのファイルも一覧に並べる。

```bash
cp example.env .env    # HOST / PORT / ORIGIN / RELAY_URL / TOKEN を書く
sfs
```

`TOKEN` は16文字以上のランダムな文字列にする(リレーと共通)。仕様と注意は [docs/SPEC.md](docs/SPEC.md)。

## 設定ファイルの場所

`.env` は、まずカレントディレクトリから探します。カレントディレクトリにあれば、それだけを読みます。なければ、実行ファイルがあるディレクトリのファイルを読みます。

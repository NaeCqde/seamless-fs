# Seamless File Server (`sfs`)

カレントディレクトリのファイルを HTTP で配り、リレー経由で他の `sfs` サーバーのファイルも一覧に並べる。

```bash
cp example.env .env    # HOST / PORT / ORIGIN / RELAY_URL / TOKEN を書く
sfs
```

`TOKEN` は16文字以上のランダムな文字列にする(リレーと共通)。仕様と注意は [docs/SPEC.md](docs/SPEC.md)。

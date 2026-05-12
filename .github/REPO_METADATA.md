# GitHub Repository Metadata

This file documents the recommended `gh repo edit` settings to make the GitHub "About" panel match the project's portfolio framing.

> Run these `gh` commands once after the docs reorg is merged. They configure description, topics, social preview, and feature toggles.

## About Description

```
Anonymous, decentralized, censorship-resistant SNS protocol. L1 blockchain (Substrate) + libp2p/Tor + client-side Wasm crypto + Next.js. Order without rulers.
```

## Topics

```
blockchain  substrate  polkadot-sdk  decentralized-social  decentralized
sns  privacy  anonymity  tor  libp2p  censorship-resistance
rust  typescript  nextjs  wasm  webassembly  cryptography
shamir-secret-sharing  kzg-commitments  stealth-address  zero-knowledge
proof-of-work  pow  randomx  end-to-end-encryption
portfolio  l1
```

## Homepage URL

(任意) ライブデモを公開した onion address or http URL を設定する。未公開なら空のままで OK。

## One-shot script

```bash
#!/usr/bin/env bash
set -euo pipefail

gh repo edit \
  --description "Anonymous, decentralized, censorship-resistant SNS protocol. L1 blockchain (Substrate) + libp2p/Tor + client-side Wasm crypto + Next.js. Order without rulers." \
  --add-topic blockchain \
  --add-topic substrate \
  --add-topic polkadot-sdk \
  --add-topic decentralized-social \
  --add-topic decentralized \
  --add-topic sns \
  --add-topic privacy \
  --add-topic anonymity \
  --add-topic tor \
  --add-topic libp2p \
  --add-topic censorship-resistance \
  --add-topic rust \
  --add-topic typescript \
  --add-topic nextjs \
  --add-topic wasm \
  --add-topic webassembly \
  --add-topic cryptography \
  --add-topic shamir-secret-sharing \
  --add-topic kzg-commitments \
  --add-topic stealth-address \
  --add-topic zero-knowledge \
  --add-topic proof-of-work \
  --add-topic randomx \
  --add-topic end-to-end-encryption \
  --add-topic portfolio \
  --add-topic l1
```

## Social Preview Image

Settings → General → Social preview に画像をアップロード。

- 推奨サイズ: 1280×640 PNG/JPG
- ソース: [assets/banner.svg](../assets/banner.svg) を 1280×640 にラスタライズ
  ```bash
  # rsvg-convert または ImageMagick 等で
  rsvg-convert -w 1280 -h 640 assets/banner.svg -o /tmp/social.png
  # その後 GitHub Web UI からアップロード (CLI からは不可)
  ```

## Feature Toggles (推奨)

| Feature | 設定 | 理由 |
|---|---|---|
| Issues | ✅ Enabled | バグ・要望受付 |
| Projects | ⚪ お好み | TODO 管理を docs/development/todo.md でやるなら無効可 |
| Wiki | ❌ Disabled | docs/ で完結させる |
| Discussions | ⚪ お好み | コミュニティ形成段階で有効化 |
| Sponsorships | ⚪ お好み | 個人開発として有効化可 |

CLI 例:

```bash
gh repo edit \
  --enable-issues=true \
  --enable-wiki=false \
  --enable-discussions=false
```

## Branch Protection (任意)

```bash
gh api repos/MoriwakiYusuke/anarchy/branches/main/protection \
  --method PUT \
  --input - <<'JSON'
{
  "required_status_checks": null,
  "enforce_admins": false,
  "required_pull_request_reviews": null,
  "restrictions": null,
  "allow_force_pushes": false,
  "allow_deletions": false
}
JSON
```

## License Detection

`LICENSE` ファイルが root に無い場合は MIT を追加すると GitHub が自動検出して About に表示します。

```bash
# 例 (MIT)
curl -fsSL https://raw.githubusercontent.com/licenses/license-templates/master/templates/mit.txt -o LICENSE
```

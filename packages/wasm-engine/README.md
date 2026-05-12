# anarchy-wasm-engine

Anarchy フロントエンドが Web Worker 上で使用する Rust → Wasm 暗号エンジン。

## 提供機能

| 機能 | 実装 |
|---|---|
| KZG-VSS hybrid (秘密分散) | `ark-bls12-381`, `ark-poly-commit` |
| Merkle Tree | `rs_merkle` + Blake2b |
| ステルスアドレス | EIP-5564 互換 |
| DM 暗号化 | encrypt / decrypt / 固定長 padding / envelope (ChaCha20-Poly1305) |
| ハッシュ | Blake2b (chain と同じ) |

## ビルド

```bash
cd packages/wasm-engine
wasm-pack build --target web --out-dir pkg
```

成果物は `pkg/` に出力され、フロントエンドが file: 依存で参照します:

```json
// apps/frontend/package.json
"anarchy-wasm-engine": "file:../../packages/wasm-engine/pkg"
```

`pnpm install` 時の postinstall (`apps/frontend/scripts/copy-wasm.sh`) で配置されます。

## 構成

```
src/
├── lib.rs          # wasm-bindgen エントリポイント
├── kzg/            # KZG-VSS hybrid scheme
├── merkle/         # Merkle tree (rs_merkle wrapper)
├── stealth/        # EIP-5564 ステルスアドレス
└── dm/             # DM 暗号化 (encrypt/decrypt/padding/envelope)

srs/                # KZG SRS (Structured Reference String)
benches/            # criterion ベンチ
tests/              # Rust ユニット + Wasm 統合テスト
```

## テスト

```bash
# Rust ユニット
cargo test --features test-utils

# Wasm 統合 (wasm-pack 経由)
wasm-pack test --headless --chrome
```

## 設計ドキュメント

- 仕様: [docs/architecture/storage-strategy.md](../../docs/architecture/storage-strategy.md), [docs/architecture/posr.md](../../docs/architecture/posr.md)
- DM 暗号化: [docs/security/dm-key-exposure.md](../../docs/security/dm-key-exposure.md)
- スキル: [.claude/skills/wasm-engine/SKILL.md](../../.claude/skills/wasm-engine/SKILL.md)

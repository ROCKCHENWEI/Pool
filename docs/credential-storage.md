# Credential Storage

## 目标

Pool Runtime 已经支持通过 `/api/api-keys` 保存 Provider、Hermes、Agent 等凭证，并在 snapshot/MCP 中只返回脱敏状态。本页说明本地凭证的落库方式。

## 存储模式

默认兼容模式：

```text
legacy-plaintext
```

如果没有配置 `POOL_CREDENTIAL_PASSPHRASE`，Runtime 会保持旧行为，把 key material 写入 SQLite 的 `api_keys.encrypted_key` 字段。这是为了兼容早期本地 smoke 和已有数据库。

本地加密模式：

```bash
export POOL_CREDENTIAL_PASSPHRASE='local development passphrase'
```

配置后，新写入的凭证会保存为：

```text
pool:v1:aes256gcm:<nonce>:<ciphertext>
```

Runtime 使用 AES-256-GCM 封装 key material，snapshot 只显示 `key_hint` 和 metadata，不返回明文。

macOS Keychain 模式：

```bash
export POOL_CREDENTIAL_STORE=keychain
# 可选：自定义服务名前缀，默认 pool-runtime。
export POOL_KEYCHAIN_SERVICE_PREFIX=pool-runtime
```

配置后，新写入的凭证会通过 macOS `security add-generic-password` 写入系统钥匙串，SQLite 的 `api_keys.encrypted_key` 只保存引用：

```text
pool:v1:keychain:<service>:<account>
```

Runtime 读取凭证时会调用 `security find-generic-password -w`。如果本机 `security` 不在 PATH，可用 `POOL_SECURITY_CLI=/usr/bin/security` 指定路径。Keychain 模式不会把明文 key material 写入 SQLite。

## Metadata

`api_keys.metadata` 会包含：

```json
{
  "credential": {
    "storage": "pool:v1:aes256gcm",
    "backend": "sqlite-encrypted",
    "encrypted": true,
    "key_hint": "...cret"
  }
}
```

Keychain 数据会标记为：

```json
{
  "credential": {
    "storage": "pool:v1:keychain",
    "backend": "macos-keychain",
    "encrypted": true,
    "key_hint": "...cret",
    "reference": {
      "service": "pool-runtime:suno:provider",
      "account": "suno/provider"
    }
  }
}
```

旧数据或未配置 passphrase 的数据会标记为：

```json
{
  "credential": {
    "storage": "legacy-plaintext",
    "backend": "sqlite",
    "encrypted": false,
    "key_hint": "...cret"
  }
}
```

## CLI 写入

`pool-cli set-api-key` 直接复用 Runtime HTTP handler 写入 `/api/api-keys`。推荐使用环境变量传入 key：

```bash
OPENAI_API_KEY=sk-test-local cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo set-api-key openai-image-2 \
  --api-key-env OPENAI_API_KEY \
  --metadata owner=local-smoke
```

读取脱敏状态：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo api-keys
```

CLI 会把 `--api-key-env` 记录为 `metadata.source=env` 和 `metadata.env=<ENV_NAME>`；snapshot、MCP resource、Web 控制台和 CLI `api-keys` 输出都不返回明文 key。

## 轮换审计

写入凭证时可以记录单个 key 的轮换周期：

```bash
OPENAI_API_KEY=sk-test-local cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo set-api-key openai-image-2 \
  --api-key-env OPENAI_API_KEY \
  --rotation-days 30 \
  --metadata owner=local-smoke
```

读取时可指定默认审计窗口：

```bash
cargo run -p pool-cli -- --db target/pool-cli-smoke/pool-runtime.sqlite --project demo api-keys --rotation-days 30
```

对应 Runtime HTTP 入口：

```text
GET /api/api-keys?rotation_days=30
```

响应会新增 `audit.kind=pool_api_key_audit`，并为每个 key 返回 `backend`、`source`、`owner`、`age_days`、`rotation_days`、`rotation_due` 和 `encrypted`。如果单个 key metadata 内有 `rotation_days`，优先使用该值；否则使用请求里的 `rotation_days`，缺省为 90 天。审计只使用 `created_at`、`updated_at` 和 metadata，不读取或返回明文 key。

## 读取规则

- 明文旧数据可以继续读取。
- 加密数据需要同一个 `POOL_CREDENTIAL_PASSPHRASE`。
- Keychain 数据需要 `POOL_CREDENTIAL_STORE=keychain`，并且运行进程能访问同一个 macOS 用户钥匙串。
- Keychain 模式如果同时设置 `POOL_CREDENTIAL_PASSPHRASE`，仍可读取历史 AES-256-GCM 数据；新写入凭证会进入 Keychain。
- 如果加密数据缺少 passphrase，Provider run 会无法解密该凭证。

## 当前边界

- Keychain 模式通过 macOS `security` CLI 实现，首版不直接链接 Security.framework。
- passphrase 模式当前用 SHA-256 派生 AES key；生产使用时应优先选择 Keychain，或把 passphrase 派生替换为更强 KDF。
- SQLite snapshot、MCP resource 和 Web 控制台不会暴露明文 key。

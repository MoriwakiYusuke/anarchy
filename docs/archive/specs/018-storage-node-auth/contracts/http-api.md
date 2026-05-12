# HTTP API Contract: Session-Authenticated Operations

> **ABANDONED (2026-03)**: セッション認証は不要と判断され撤去済み。詳細は [../spec.md](../spec.md) の冒頭を参照。

**Version**: 1.0.0  
**Base URL**: `http://localhost:3030`  
**Content-Type**: `application/json`

## Overview

セッショントークン認証を必要とするHTTP API。
フラグメント書き込み・削除操作にはセッショントークンが必須。

## Authentication

### Session Token Header

書き込み・削除操作には`X-Session-Token`ヘッダーが必須。

```http
X-Session-Token: a1b2c3d4e5f6g7h8...
```

| Header | Required | Description |
|--------|----------|-------------|
| X-Session-Token | Yes (write/delete) | 64文字のhexエンコードトークン |

### Authentication Flow

1. ブロックチェーンノードがlibp2p経由で`storage_requestSession`を呼び出し
2. セッショントークンを取得
3. HTTP API呼び出し時に`X-Session-Token`ヘッダーを付与
4. ストレージノードがトークンを検証

---

## Endpoints

### Health Check

**認証不要**

```http
GET /health
```

**Response (200 OK)**:

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

### Write Fragment

**認証必須**

```http
POST /fragments
X-Session-Token: a1b2c3d4...
Content-Type: application/json

{
  "id": "fragment_abc123",
  "data": "base64-encoded-data...",
  "commitment": "0x...",
  "metadata": {
    "post_id": "post_xyz789",
    "shard_index": 0,
    "total_shards": 5
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| id | `string` | Yes | フラグメントID（ユニーク） |
| data | `string` | Yes | Base64エンコードデータ |
| commitment | `string` | Yes | KZGコミットメント（hex） |
| metadata | `object` | Yes | フラグメントメタデータ |
| metadata.post_id | `string` | Yes | 所属する投稿ID |
| metadata.shard_index | `integer` | Yes | シャードインデックス |
| metadata.total_shards | `integer` | Yes | 総シャード数 |

**Response (201 Created)**:

```json
{
  "id": "fragment_abc123",
  "stored_at": 1709251200,
  "size_bytes": 1024
}
```

**Response (401 Unauthorized)**:

```json
{
  "error": "missing_token",
  "message": "X-Session-Token header is required"
}
```

**Response (403 Forbidden)**:

```json
{
  "error": "invalid_token",
  "message": "Session token is invalid or expired"
}
```

**Response (409 Conflict)**:

```json
{
  "error": "fragment_exists",
  "message": "Fragment with this ID already exists"
}
```

---

### Read Fragment

**認証不要**（読み取りは誰でも可能）

```http
GET /fragments/{id}
```

**Response (200 OK)**:

```json
{
  "id": "fragment_abc123",
  "data": "base64-encoded-data...",
  "commitment": "0x...",
  "metadata": {
    "post_id": "post_xyz789",
    "shard_index": 0,
    "total_shards": 5
  },
  "stored_at": 1709251200
}
```

**Response (404 Not Found)**:

```json
{
  "error": "not_found",
  "message": "Fragment not found"
}
```

---

### Delete Fragment

**認証必須**

```http
DELETE /fragments/{id}
X-Session-Token: a1b2c3d4...
```

**Response (204 No Content)**:

（ボディなし）

**Response (401 Unauthorized)**:

```json
{
  "error": "missing_token",
  "message": "X-Session-Token header is required"
}
```

**Response (403 Forbidden)**:

```json
{
  "error": "invalid_token",
  "message": "Session token is invalid or expired"
}
```

**Response (404 Not Found)**:

```json
{
  "error": "not_found",
  "message": "Fragment not found"
}
```

---

### List Fragments

**認証不要**

```http
GET /fragments?post_id={post_id}&limit={limit}&offset={offset}
```

| Query Param | Type | Required | Default | Description |
|-------------|------|----------|---------|-------------|
| post_id | `string` | No | - | 投稿IDでフィルタ |
| limit | `integer` | No | 100 | 最大件数（1-1000） |
| offset | `integer` | No | 0 | オフセット |

**Response (200 OK)**:

```json
{
  "fragments": [
    {
      "id": "fragment_abc123",
      "post_id": "post_xyz789",
      "shard_index": 0,
      "size_bytes": 1024,
      "stored_at": 1709251200
    }
  ],
  "total": 5,
  "limit": 100,
  "offset": 0
}
```

---

## Error Responses

### Common Error Schema

```json
{
  "error": "error_code",
  "message": "Human readable message"
}
```

### HTTP Status Codes

| Status | Error Code | Description |
|--------|------------|-------------|
| 400 | `bad_request` | リクエストボディが不正 |
| 401 | `missing_token` | X-Session-Tokenヘッダーがない |
| 403 | `invalid_token` | トークンが無効または期限切れ |
| 404 | `not_found` | リソースが見つからない |
| 409 | `conflict` | リソースが既に存在 |
| 500 | `internal_error` | サーバー内部エラー |

---

## OpenAPI Schema

```yaml
openapi: 3.0.3
info:
  title: Anarchy Storage Node API
  version: 1.0.0
  description: Session-authenticated storage node HTTP API

security: []

paths:
  /health:
    get:
      summary: Health check
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                  version:
                    type: string

  /fragments:
    get:
      summary: List fragments
      parameters:
        - name: post_id
          in: query
          schema:
            type: string
        - name: limit
          in: query
          schema:
            type: integer
            default: 100
        - name: offset
          in: query
          schema:
            type: integer
            default: 0
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/FragmentList'

    post:
      summary: Write fragment
      security:
        - sessionToken: []
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/FragmentWrite'
      responses:
        '201':
          description: Created
        '401':
          $ref: '#/components/responses/Unauthorized'
        '403':
          $ref: '#/components/responses/Forbidden'

  /fragments/{id}:
    get:
      summary: Read fragment
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        '200':
          description: OK
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Fragment'
        '404':
          $ref: '#/components/responses/NotFound'

    delete:
      summary: Delete fragment
      security:
        - sessionToken: []
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        '204':
          description: No Content
        '401':
          $ref: '#/components/responses/Unauthorized'
        '403':
          $ref: '#/components/responses/Forbidden'
        '404':
          $ref: '#/components/responses/NotFound'

components:
  securitySchemes:
    sessionToken:
      type: apiKey
      in: header
      name: X-Session-Token
      description: 64-character hex session token

  schemas:
    Fragment:
      type: object
      properties:
        id:
          type: string
        data:
          type: string
          format: byte
        commitment:
          type: string
        metadata:
          $ref: '#/components/schemas/FragmentMetadata'
        stored_at:
          type: integer

    FragmentMetadata:
      type: object
      properties:
        post_id:
          type: string
        shard_index:
          type: integer
        total_shards:
          type: integer

    FragmentWrite:
      type: object
      required:
        - id
        - data
        - commitment
        - metadata
      properties:
        id:
          type: string
        data:
          type: string
          format: byte
        commitment:
          type: string
        metadata:
          $ref: '#/components/schemas/FragmentMetadata'

    FragmentList:
      type: object
      properties:
        fragments:
          type: array
          items:
            type: object
            properties:
              id:
                type: string
              post_id:
                type: string
              shard_index:
                type: integer
              size_bytes:
                type: integer
              stored_at:
                type: integer
        total:
          type: integer
        limit:
          type: integer
        offset:
          type: integer

    Error:
      type: object
      properties:
        error:
          type: string
        message:
          type: string

  responses:
    Unauthorized:
      description: Missing session token
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'
    Forbidden:
      description: Invalid or expired session token
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'
    NotFound:
      description: Resource not found
      content:
        application/json:
          schema:
            $ref: '#/components/schemas/Error'
```

---

## Implementation Notes

### Middleware Pattern (axum)

```rust
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

pub async fn require_session(
    State(registry): State<Arc<SessionRegistry>>,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let token = request
        .headers()
        .get("X-Session-Token")
        .and_then(|v| v.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "missing_token".into(),
                message: "X-Session-Token header is required".into(),
            }),
        ))?;

    registry
        .validate(token)
        .ok_or((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "invalid_token".into(),
                message: "Session token is invalid or expired".into(),
            }),
        ))?;

    // Update last_access for idle timeout
    registry.touch(token);

    Ok(next.run(request).await)
}
```

### Router Setup

```rust
use axum::{middleware, Router};

let app = Router::new()
    // Public endpoints (no auth)
    .route("/health", get(health_handler))
    .route("/fragments", get(list_fragments))
    .route("/fragments/:id", get(read_fragment))
    // Protected endpoints (require session)
    .route("/fragments", post(write_fragment))
    .route("/fragments/:id", delete(delete_fragment))
    .route_layer(middleware::from_fn_with_state(
        registry.clone(),
        require_session,
    ));
```

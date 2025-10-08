# Portable API Request Pattern

## Overview

API request types are defined once in `crates/service/src/http_server/api/v0/{resource}/{operation}.rs` and used in three contexts:

1. **HTTP API handlers** - Axum handlers deserialize from JSON
2. **HTTP client requests** - `ApiRequest` trait builds HTTP requests
3. **CLI arguments** - clap derives parse command-line args (via optional `clap` feature)

Request types are pure API types with no CLI-specific concerns. CLI configuration (like `--remote`) is handled via `OpContext` injected at the top level.

## Structure

```
crates/
├── service/
│   └── src/
│       └── http_server/
│           └── api/
│               ├── client/           # ApiClient + ApiRequest trait
│               └── v0/
│                   └── bucket/
│                       ├── create.rs # CreateRequest + handler + ApiRequest impl
│                       └── list.rs   # ListRequest + handler + ApiRequest impl
└── app/
    └── src/
        └── ops/
            └── bucket/
                ├── mod.rs           # Nested subcommands using command_enum!
                ├── create.rs        # Op impl for CreateRequest
                └── list.rs          # Op impl for ListRequest
```

## Implementation

### 1. Define Request/Response in API Handler File

`crates/service/src/http_server/api/v0/bucket/create.rs`:

```rust
use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

// Request type with conditional clap support
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct CreateRequest {
    /// Name of the bucket to create
    #[cfg_attr(feature = "clap", arg(long))]
    pub name: String,

    /// Optional region
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "clap", arg(long))]
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResponse {
    pub bucket_id: String,
    pub name: String,
    pub created_at: String,
}

// HTTP handler
pub async fn handler(
    State(_state): State<ServiceState>,
    Json(req): Json<CreateRequest>,
) -> Result<impl IntoResponse, CreateError> {
    let bucket_id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    Ok((
        http::StatusCode::CREATED,
        Json(CreateResponse {
            bucket_id,
            name: req.name,
            created_at,
        }),
    ))
}

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("Bucket already exists: {0}")]
    AlreadyExists(String),
    #[error("Invalid bucket name: {0}")]
    InvalidName(String),
}

impl IntoResponse for CreateError {
    fn into_response(self) -> Response {
        match self {
            CreateError::AlreadyExists(name) => (
                http::StatusCode::CONFLICT,
                format!("Bucket already exists: {}", name),
            ).into_response(),
            CreateError::InvalidName(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid name: {}", msg)
            ).into_response(),
        }
    }
}

// Client implementation - builds HTTP request
impl ApiRequest for CreateRequest {
    type Response = CreateResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket").unwrap();
        client.post(full_url).json(&self)
    }
}
```

### 2. Enable clap Feature

`crates/service/Cargo.toml`:
```toml
[dependencies]
clap = { workspace = true, optional = true }

[features]
clap = ["dep:clap"]
```

`crates/app/Cargo.toml`:
```toml
[dependencies]
service = { path = "../service", features = ["clap"] }
```

### 3. Implement Op for Request Type (with Context Injection)

`crates/app/src/ops/bucket/create.rs`:

```rust
use service::http_server::api::client::{ApiClient, ApiError, ApiRequest};
use service::http_server::api::v0::bucket::create::{CreateRequest, CreateResponse};

#[derive(Debug, thiserror::Error)]
pub enum BucketCreateError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Bucket operation failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for CreateRequest {
    type Error = BucketCreateError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        if let Some(remote) = &ctx.remote {
            // Remote execution - send HTTP request
            let mut client = ApiClient::new(remote)
                .map_err(|e| BucketCreateError::Failed(e.to_string()))?;

            let response: CreateResponse = client.call(self.clone()).await?;

            Ok(format!(
                "Created bucket: {} (id: {}) at {}",
                response.name, response.bucket_id, response.created_at
            ))
        } else {
            // Local execution
            Ok(format!("Would create bucket locally: {}", self.name))
        }
    }
}
```

`crates/app/src/op.rs`:

```rust
#[derive(Debug, Clone)]
pub struct OpContext {
    pub remote: Option<String>,
}

#[async_trait::async_trait]
pub trait Op: Send + Sync {
    type Error: Error + Send + Sync + 'static;
    type Output;

    async fn execute(&self, ctx: &OpContext) -> Result<Self::Output, Self::Error>;
}
```

### 4. Add Global CLI Flags

`crates/app/src/args.rs`:

```rust
#[derive(Parser, Debug)]
pub struct Args {
    /// Remote server URL (if not provided, executes locally)
    #[arg(long, global = true)]
    pub remote: Option<String>,

    #[command(subcommand)]
    pub command: crate::Command,
}
```

`crates/app/src/main.rs`:

```rust
#[tokio::main]
async fn main() {
    let args = Args::parse();

    let ctx = op::OpContext {
        remote: args.remote,
    };

    match args.command.execute(&ctx).await {
        Ok(output) => {
            println!("{}", output);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
```

### 5. Wire Up Nested Subcommands

`crates/app/src/ops/bucket/mod.rs`:

```rust
use clap::{Args, Subcommand};

pub mod create;
pub mod list;

use crate::op::Op;
use service::http_server::api::v0::bucket::{CreateRequest, ListRequest};

crate::command_enum! {
    (Create, CreateRequest),
    (List, ListRequest),
}

pub type BucketCommand = Command;

#[derive(Args, Debug, Clone)]
pub struct Bucket {
    #[command(subcommand)]
    pub command: BucketCommand,
}

#[async_trait::async_trait]
impl Op for Bucket {
    type Error = OpError;
    type Output = OpOutput;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        self.command.execute(ctx).await
    }
}
```

## Usage

### CLI (local execution)
```bash
cli bucket create --name my-bucket --region us-west
```

### CLI (remote execution via --remote flag)
```bash
cli --remote http://localhost:3000 bucket create --name my-bucket --region us-west
```

### HTTP API
```bash
curl -X POST http://localhost:3000/api/v0/bucket \
  -H "Content-Type: application/json" \
  -d '{"name":"my-bucket","region":"us-west"}'
```

### HTTP Client (from code)
```rust
use service::http_server::api::client::ApiClient;
use service::http_server::api::v0::bucket::create::CreateRequest;

let mut client = ApiClient::new("http://localhost:3000")?;
let request = CreateRequest {
    name: "my-bucket".to_string(),
    region: Some("us-west".to_string()),
};
let response = client.call(request).await?;
```

## Key Benefits

1. **Single source of truth** - Request/response types defined once
2. **Type safety** - Shared types between CLI, HTTP, and client
3. **Zero duplication** - Same struct for clap Args and serde JSON
4. **Minimal boilerplate** - One file per operation
5. **Clean separation** - CLI concerns (like `--remote`) separate from API types
6. **Context injection** - `OpContext` provides configuration to ops without polluting request types
7. **Easy to add** - New operations follow same simple pattern

## Adding a New Operation

1. Create `crates/service/src/http_server/api/v0/{resource}/{operation}.rs`
   - Add request/response types with `#[cfg_attr(feature = "clap", derive(clap::Args))]`
   - Implement HTTP handler
   - Implement `ApiRequest` trait

2. Create `crates/app/src/ops/{resource}/{operation}.rs`
   - Implement `Op` trait for the request type

3. Update `crates/app/src/ops/{resource}/mod.rs`
   - Add operation to `command_enum!`

Done!

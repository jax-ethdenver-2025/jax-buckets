# Usage Guide

This guide covers how to use JaxBucket for creating encrypted storage buckets and syncing between peers.

## Prerequisites

Before using JaxBucket, make sure you have:

1. **Installed JaxBucket** - See [INSTALL.md](INSTALL.md)
2. **Initialized configuration** - Run `jax init`
3. **Started the service** - Run `jax service`

## CLI Overview

The `jax` CLI provides commands for managing buckets and interacting with the service:

```bash
jax [OPTIONS] <COMMAND>

Commands:
  bucket   # Bucket operations (create, list, add, ls, cat, mount, share)
  init     # Initialize configuration
  service  # Start the JaxBucket service
  version  # Show version information
```

**Global Options:**
- `--remote <URL>` - API endpoint (default: `http://localhost:3000`)
- `--config-path <PATH>` - Custom config directory (default: `~/.config/jax`)

## Bucket Operations

### Create a Bucket

Create a new encrypted bucket:

```bash
jax bucket create --name my-bucket
```

This creates a new bucket and returns its UUID.

### List Buckets

View all buckets:

```bash
jax bucket list
```

Returns a JSON array of buckets with their IDs, names, and metadata.

### Add Files to a Bucket

Add a file or directory to a bucket:

```bash
# Add a single file
jax bucket add --name my-bucket --path /local/path/to/file.txt

# Add a directory
jax bucket add --name my-bucket --path /local/path/to/directory
```

Files are automatically encrypted and stored in the bucket.

### List Bucket Contents

View the contents of a bucket:

```bash
jax bucket ls --name my-bucket
```

Shows the directory tree of the bucket.

### View File Contents

Download and view a file from a bucket:

```bash
jax bucket cat --name my-bucket --path /path/in/bucket/file.txt
```

The file is decrypted and output to stdout.

### Share a Bucket

Share a bucket with another peer:

```bash
jax bucket share --bucket-id <bucket-id> --peer-public-key <recipient-node-id>
```

## Web UI

The web interface provides a graphical way to interact with JaxBucket.

### Dashboard

Navigate to `http://localhost:8080` to see:
- List of all your buckets
- Bucket information (ID, name, size)
- Sync status
- Your Node ID

### Bucket Explorer

Click on a bucket to browse its contents:
- View directory structure
- Upload files
- Download files
- View file metadata

### File Viewer

Click on a file to:
- View file contents (for text files)
- Download the file
- See MIME type and metadata

## Working with Multiple Peers

### Get Your Node ID

Your Node ID is your public key that other peers use to share buckets with you:

```bash
# View in the web UI at http://localhost:8080
# Or check the service startup output when you run `jax service`
```

The Node ID is displayed in the format: `<hex-encoded-public-key>`

### Share a Bucket with Another Peer

1. **Get recipient's Node ID** from them (out-of-band, e.g., via email, QR code)
2. **Share the bucket:**
   ```bash
   jax bucket share <bucket-id> --peer-id <their-node-id> --role editor
   ```
3. **Recipient will automatically receive the bucket** on their next sync

### Sync Buckets

JaxBucket automatically syncs in the background, but you can also use the web UI to monitor sync status.

# Installation Guide

This guide covers installation and system requirements for JaxBucket.

## System Requirements

### Operating Systems

- **Linux**: Any modern distribution (Ubuntu 20.04+, Debian 11+, Fedora 35+, etc.)
- **macOS**: 10.15 (Catalina) or later
- **Windows**: Windows 10/11 with WSL2 recommended (native Windows support is experimental)

### Software Requirements

- **Rust**: Version 1.75 or later (2021 edition)
- **Cargo**: Comes with Rust installation
- **Git**: For cloning the repository

### System Libraries

JaxBucket requires the following system libraries:

#### Linux (Ubuntu/Debian)
```bash
sudo apt update
sudo apt install build-essential pkg-config libssl-dev libsqlite3-dev
```

#### Linux (Fedora/RHEL)
```bash
sudo dnf install gcc pkg-config openssl-devel sqlite-devel
```

#### macOS
```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies (most come with Xcode Command Line Tools)
brew install openssl sqlite3
```

#### Windows (WSL2)
Follow the Linux (Ubuntu/Debian) instructions above within your WSL2 environment.

### Hardware Requirements

**Minimum:**
- CPU: 2 cores
- RAM: 2 GB
- Disk: 500 MB for binaries + storage for your encrypted data

**Recommended:**
- CPU: 4+ cores
- RAM: 4+ GB
- Disk: 10+ GB for comfortable operation
- Network: Stable internet connection for P2P sync

## Installation

### Option 1: Install from Crates.io (Recommended)

Once published, you can install JaxBucket directly from crates.io:

```bash
cargo install jax-bucket
```

This will download, compile, and install the `jax` binary to `~/.cargo/bin/`.

### Option 2: Install from Git Repository

Install the latest development version:

```bash
cargo install --git https://github.com/jax-ethdenver-2025/jax-bucket jax-bucket
```

### Option 3: Build from Source

Clone and build manually for development or customization:

```bash
# Clone the repository
git clone https://github.com/jax-ethdenver-2025/jax-bucket.git
cd jax-bucket

# Build in release mode
cargo build --release

# Install to ~/.cargo/bin
cargo install --path crates/app

# Or run directly from the build directory
./target/release/jax --help
```

### Verify Installation

After installation, verify that `jax` is in your PATH:

```bash
jax --help
```

You should see output like:
```
A basic CLI example

Usage: jax [OPTIONS] <COMMAND>

Commands:
  bucket
  init
  service
  version
  help     Print this message or the help of the given subcommand(s)
```

If the command is not found, ensure `~/.cargo/bin` is in your PATH:

```bash
# Add to your shell profile (.bashrc, .zshrc, etc.)
export PATH="$HOME/.cargo/bin:$PATH"
```

## Initial Setup

### 1. Initialize Configuration

Create the configuration directory and generate your identity:

```bash
jax init
```

This creates:
- `~/.config/jax/` - Configuration directory (or custom path if specified with `--config-path`)
- `config.toml` - Service configuration
- `secret.pem` - Your Ed25519 identity keypair (keep this secure!)
- `jax.db` - SQLite database for bucket metadata
- `blobs/` - Directory for encrypted blob storage

**Security Note:** The `secret.pem` file contains your private key. Keep it secure and back it up safely. Anyone with access to this file can decrypt your buckets and impersonate you.

### 2. Configure Service (Optional)

The default configuration works out of the box, but you can customize settings by editing the generated `config.toml`:

```toml
[node]
# Path to your identity key
secret_key_path = "secret.pem"

# Path to blob storage
blobs_path = "blobs"

# Network bind port (0 = random ephemeral port)
bind_port = 0

[database]
# SQLite database path
path = "db.sqlite"

[http_server]
# API server listen address
api_addr = "127.0.0.1:3000"

# Web UI listen address
html_addr = "127.0.0.1:8080"
```

### 3. Start the Service

```bash
jax service
```

The service will:
- Start the HTTP API server on `http://localhost:3000`
- Start the Web UI server on `http://localhost:8080`
- Initialize the Iroh P2P node
- Begin listening for sync events
- Display your Node ID (public key)

Keep this running in a terminal, or run it as a background service (see below).

### 4. Access the Web UI

Open your browser and navigate to:
```
http://localhost:8080
```

You should see the JaxBucket dashboard.

## Running as a Background Service

### Linux (systemd)

Create a systemd service file at `~/.config/systemd/user/jaxbucket.service`:

```ini
[Unit]
Description=JaxBucket P2P Storage Service
After=network.target

[Service]
Type=simple
ExecStart=%h/.cargo/bin/jax service
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=default.target
```

Enable and start the service:
```bash
systemctl --user enable jaxbucket
systemctl --user start jaxbucket

# Check status
systemctl --user status jaxbucket

# View logs
journalctl --user -u jaxbucket -f
```

### macOS (launchd)

Create a launch agent at `~/Library/LaunchAgents/com.jaxbucket.service.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.jaxbucket.service</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Users/YOUR_USERNAME/.cargo/bin/jax</string>
        <string>service</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/jaxbucket.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/jaxbucket.err</string>
</dict>
</plist>
```

Load the service:
```bash
launchctl load ~/Library/LaunchAgents/com.jaxbucket.service.plist

# Check status
launchctl list | grep jaxbucket

# View logs
tail -f /tmp/jaxbucket.log
```

## Troubleshooting

### "Command not found: jax"

Ensure `~/.cargo/bin` is in your PATH:
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### "Permission denied" on secret.pem

Fix file permissions:
```bash
chmod 600 ~/.config/jax/secret.pem
```

### "Database is locked"

Only one instance of `jax service` can run at a time. Stop any existing instances:
```bash
pkill -f "jax service"
```

### "Failed to bind address"

The HTTP port is already in use. Change it in `config.toml` or stop the conflicting service.

### Reset Configuration

To start fresh:
```bash
# Backup first if needed
mv ~/.config/jax ~/.config/jax.backup

# Reinitialize
jax init
```

## Next Steps

- See [USAGE.md](USAGE.md) for how to use JaxBucket
- Read [PROTOCOL.md](PROTOCOL.md) to understand how JaxBucket works internally
- Check [DEVELOPMENT.md](DEVELOPMENT.md) for development and contribution guidelines

## Getting Help

- **Documentation**: https://docs.rs/jax-bucket
- **Issues**: https://github.com/jax-ethdenver-2025/jax-bucket/issues
- **Discussions**: https://github.com/jax-ethdenver-2025/jax-bucket/discussions

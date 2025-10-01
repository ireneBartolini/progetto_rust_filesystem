# progetto_rust_filesystem

# Overview
This project aims to implement a remote file system client in Rust that presents a local mount point, mirroring the struc
file system hosted on a remote server. The file system should support transparent read and write access to remote file
# Goals
- Provide a local file system interface that interacts with a remote storage backend.
- Enable standard file operations (read, write, create, delete, etc.) on remote files as if they were local.
- Ensure compatibility with Linux systems.
- Optionally support Windows and macOS with best-effort 
# Funtional Requirements
- Mount a virtual file system to a local path (e.g., /mnt/remote-fs )
- Display directories and files from a remote source
Read files from the remote server
- Write modified files back to the remote server
- Support creation, deletion, and renaming of files and directories
- Maintain file attributes such as size, timestamps, and permissions (as feasible)
- Run as a background daemon process that handles filesystem operations continuously

# Server Interface and Implementation 
- The server should offer a set RESTful API for file operations:
GET /list/<path> – List directory contents
GET /files/<path> – Read file contents
PUT /files/<path> – Write file contents
POST /mkdir/<path> – Create directory
DELETE /files/<path> – Delete file or directory
- The server can be implemented using any language or framework, but should be RESTful and stateless.

# Caching
- Optional local caching layer for performance
- Configurable cache invalidation strategy (e.g., TTL or LRU)

# Performance
- Support for large files (100MB+) with streaming read/write
- Reasonable latency (<500ms for operations under normal network conditions)

# CHIAMATE API

## List directory contents

## read file content 
curl -X GET  http://127.0.0.1:8080/files/nuova_dir/dir_0/text.txt

## write file content
curl -X PUT http://127.0.0.1:8080/files/nuova_dir/dir_0/../ludo.txt      -H "Content-Type: text/plain"      -d "Ludo entra in cucina\n cade una madonna\nfine"

## make dir 
curl -X POST http://127.0.0.1:8080/mkdir/nuova_dir

## delete 
curl -X DELETE http://127.0.0.1:8080/files/nuova_dir

## test
Run on one terminal "cargo run"
Run on the other terminal "cargo test --test api_test"
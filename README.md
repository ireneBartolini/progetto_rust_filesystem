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
responce json let file_info = FileInfo::new(
                        permissions,
                        owner,
                        size,
                        formatted_time,
                        file_name.clone(),
                        is_directory,
                    );
GET /files/<path> – Read file contents
PUT /files/<path> – Write file contents
POST /mkdir/<path> – Create directory
DELETE /files/<path> – Delete file or directory
GET /lookup/<path>
- The server can be implemented using any language or framework, but should be RESTful and stateless.

# Caching
- Optional local caching layer for performance
- Configurable cache invalidation strategy (e.g., TTL or LRU)

# Performance
- Support for large files (100MB+) with streaming read/write
- Reasonable latency (<500ms for operations under normal network conditions)

# CHIAMATE API

## List directory contents
curl -X GET http://127.0.0.1:8080/list/ \
  -H "Authorization: Bearer $TOKEN_ALICE"

## read file content 
curl -X GET  http://127.0.0.1:8080/files/nuova_dir/dir_0/text.txt

## write file content
curl -X PUT http://127.0.0.1:8080/files/alice_secret.txt \
  -H "Authorization: Bearer $TOKEN_ALICE" \
  -d "This is Alice's private file!"

## make dir 
curl -X POST http://127.0.0.1:8080/mkdir/alice_documents \
  -H "Authorization: Bearer $TOKEN_ALICE"
  
## delete 
curl -X DELETE http://127.0.0.1:8080/files/alice_diary.txt \
  -H "Authorization: Bearer $TOKEN_ALICE"

## register user
curl -X POST http://127.0.0.1:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "password": "password123"}'

## login user
curl -X POST http://127.0.0.1:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "password": "password123"}'

## in order to save the token
TOKEN_ALICE=$(curl -s -X POST http://127.0.0.1:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "password": "password123"}' | \
  grep -o '"token":"[^"]*"' | cut -d'"' -f4)


  ### PER WINDOWS 
  Invoke-WebRequest `
>>   -Uri "http://127.0.0.1:8080/auth/register" `
>>   -Method POST `
>>   -Headers @{ "Content-Type" = "application/json" } `
>>   -Body '{"username": "alice", "password": "password123"}'
## write file
Invoke-RestMethod `
  -Uri "http://127.0.0.1:8080/files/alice_secret.txt" `
  -Method PUT `
  -Headers @{ "Authorization" = "Bearer ALICE_TOKEN_HERE" } `
  -Body "This is Alice's private file!"


  

## test
Run on one terminal "cargo run"
Run on the other terminal "cargo test --test api_test"



## note
FILE:
| File_ID* | Path                      | User_ID | User_Permissions | Group_Permissions | Others_Permissions | Size (bytes) | Created_At           | Last_modified         |
|----------|---------------------------|---------|------------------|-------------------|--------------------|--------------|----------------------|----------------------|
| 1        | alice/alice_secret.txt    | 1       | rw-              | r--               | ---                | 1024         | 2024-05-01 10:00:00  | 2024-06-01 09:00:00  |
| 2        | bob/bob_diary.txt         | 2       | rw-              | r--               | ---                | 2048         | 2024-05-02 11:00:00  | 2024-06-02 08:30:00  |
| 3        | shared/group_notes.txt    | 1       | rw-              | rw-               | r--                | 4096         | 2024-05-03 12:00:00  | 2024-06-03 07:45:00  |
| 4        | charlie/charlie_todo.txt  | 3       | rw-              | r--               | ---                | 512          | 2024-05-04 13:00:00  | 2024-06-04 07:00:00  |
| 5        | public/readme.txt         | 4       | rw-              | rw-               | r--                | 256          | 2024-05-05 14:00:00  | 2024-06-05 06:30:00  |

USER:
| User_ID* | Username | Password                          |
|----------|----------|-----------------------------------|
| 1        | alice    | $2b$12$abcdehashedalicepassword   |
| 2        | bob      | $2b$12$xyz12hashedbobpassword     |
| 3        | charlie  | $2b$12$mnopqhashedcharliepassword |
| 4        | dave     | $2b$12$rstuvhasheddavepassword    |

> Le password sono hashate.




#### CLIENT

- list

ls 
1. getattr <- info su directory corrente
2. readdir 
3. lookup <- su ogni entry restituita per mostrre gli attributi

ls {subdir}
1. lookup <- deve risolvere subdir 
2. getattr <- controlla se esiste e che tipo di file 
3. readdir 
4. lookup risultato <- per ogni entry

cat file.txt

1.	lookup(parent=1, name="file.txt")	Risolvi il file.
2.	getattr(ino=file_ino)	Ottieni attributi.
3.	open(ino=file_ino)	Apre il file.
4.	read(ino=file_ino, offset, size)	Legge i dati (una o più volte).
5.	release(ino=file_ino)	Chiude il file.

echo ".." > file.txt
1. lookup	<- controlla se il file è già noto	chiama /exists?path=pino.txt o restituisci ENOENT
create	crea solo inode fittizio	non fare nulla sul server
2. write	<- invia direttamente PUT /files con path e data	il server creerà o aggiornerà il file
3. getattr	restituisci attributi 

COSE DA FARE 
- sistemare la write di un file già esistenete <- penso sia un problema di attributi che la fopen dovrebbe verificare
- fare un file a parte con il RemoteFS per rendere tutto più ordinato
- cd va implementata? NAVIGARE il fs in generale 
- problema se faccio login con un altro utente il mount non si "svuota" le cartelle restano 

PROBLEMI 

- SERVER se ci sono le cartelle già presenti il server quando fa il mount non scrive i file coi permessi nel db e non funzionano
- CLIENT non si può modificare un file esistente 

COME TESTARE:
**/server cargo run 
**/client cargo run 
**/client/mount <op. filesystem>
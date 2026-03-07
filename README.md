# Remote Filesystem Rust

Progetto didattico in Rust che implementa un filesystem remoto con due componenti:

- `server`: espone API HTTP stateless con autenticazione JWT
- `client`: monta il filesystem remoto in locale tramite FUSE

L'obiettivo e rendere i file remoti accessibili come una cartella locale sulla macchina del client, mantenendo dati, logica di autenticazione, autorizzazione e metadati lato server.

## Panoramica Architetturale

### Server (`/server`)

- Framework web: Axum
- Protocollo: HTTP REST
- Auth: JWT (login) + bcrypt (hash password)
- Storage metadati: SQLite (`USER`, `METADATA`)
- Storage file: filesystem locale organizzato per utente (`server/database/remote-fs/<username>/...`) filesystem persistente dove i file di ogni utente sono mantenuti in una cartella chiamata col suo username.  I metadati di ogni file/directory sono mantenuti nel database.

Responsabilita principali:

- registrazione e login utenti
- validazione token JWT
- operazioni file/cartelle (create, list, read, write, delete, lookup)
- controllo permessi Unix-style (owner/group/others)

### Client (`/client`)

- Filesystem userspace: FUSE (`fuser`)
- Trasporto: richieste HTTP al server
- Cache locale: TTL su attributi/contenuti
- Modalita esecuzione: foreground o daemon

Responsabilita principali:

- tradurre le syscall FUSE in chiamate HTTP
- gestire token JWT e sessione utente
- mantenere una cache per ridurre roundtrip sul server

## Flusso Operativo

1. L'utente avvia il server.
2. L'utente avvia il client e inserisce credenziali.
3. Il client ottiene un token JWT dal server.
4. Il mount FUSE espone una directory locale (`mount/`).
5. Operazioni come `ls`, `cat`, `mkdir`, `rm` diventano chiamate HTTP alle API del server.

## Struttura del Repository

```text
progetto_rust_filesystem/
  server/
    src/
      main.rs
      auth.rs
      filesystem.rs
    database/
      remote-fs/
    tests/
      api_test.rs
      filesystem_test.rs

  client/
    src/
      main.rs
      fuse.rs
```

## Avvio Rapido

### 1. Avviare il server

```bash
cd server
cargo run 
```

Server in ascolto di default su `http://0.0.0.0:8080`.

### 2. Avviare il client FUSE

```bash
cd client
cargo run 
```

Modalita daemon:

```bash
cd client
cargo run -- --daemon
```

Configurazione endpoint server:

```bash
cd client
cargo run -- --server-ip 192.168.1.100 --server-port 9000
```

## API HTTP Principali

### Autenticazione

- `POST /auth/register`
- `POST /auth/login`

### File e directory

- `GET /list/<path>`: lista contenuti directory
- `GET /files/<path>`: lettura file
- `PUT /files/<path>`: scrittura/creazione file
- `POST /mkdir/<path>`: creazione directory
- `DELETE /files/<path>`: eliminazione file o directory
- `GET /lookup/<path>`: metadati nodo

## Permessi

Il progetto usa permessi Unix in formato ottale (es. `644`, `755`, `600`) e verifica accessi su owner/group/others.


## Sicurezza (Note)

Progetto nato per scopi accademici. Per uso produzione andrebbero aggiunti almeno:

- secret JWT non hardcoded
- HTTPS
- rate limiting
- validazioni input piu strette
- audit logging

## Tecnologie
- Rust
- Axum
- Tokio
- Reqwest
- FUSE (`fuser`)
- SQLite (`rusqlite`)
- JWT
- bcrypt

## Autori

- Alessandro Benvenuti - Politecnico di Torino
- Irene Bartolini - Politecnico di Torino
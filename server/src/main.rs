use server::FileSystem;
mod auth;
use auth::{AuthService, LoginRequest, RegisterRequest, AuthResponse};

use std::sync::{Arc, Mutex};
use std::path::Path as StdPath;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post, put, delete},
    Router,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // creation of the auth service
    let auth_service = Arc::new(AuthService::new());

    // The access to the FileSystem is handled through a Mutex, in order to avoid concurrent accesses
    // let fs = Arc::new(Mutex::new(FileSystem::from_file_system("remote-fs")));
    // fs.lock().unwrap().set_side_effects(true);


     let app = Router::new()
        // Route di autenticazione (pubbliche)
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        
        // Route del filesystem (protette)
        .route("/list", get(list_dir_with_empty_path))    // Handler che passa path vuoto
        .route("/list/", get(list_dir_with_empty_path))
        .route("/list/*path", get(list_dir))
        .route("/files/*path", get(read_file).put(write_file).delete(delete_file))
        .route("/mkdir/*path", post(mkdir))
        
        // Stato condiviso
        .with_state(auth_service); // The state is passed to the handlers;

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("Server listening on {}", addr);
    
    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app.into_make_service(),
    )
    .await
    .unwrap();
}

// function to create the file system
fn create_user_filesystem(username: &str) -> Result<FileSystem, String> {
    let user_path = format!("remote-fs/{}", username);
    let mut fs = FileSystem::from_file_system(&user_path);
    fs.set_side_effects(true);
    Ok(fs)
}

async fn register(
    State(auth_service): State<Arc<AuthService>>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    match auth_service.register(req) {
        Ok(message) => {
            // Salva utenti su file
            let _ = auth_service.save_to_file("users.json");
            (StatusCode::CREATED, message).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

// FUNCTION TO EXCTRACT A USER
fn extract_user_from_headers(headers: &HeaderMap, auth_service: &AuthService) -> Result<String, String> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    println!("Auth header: {:?}", auth_header);
    
    let header = auth_header.ok_or("Missing Authorization header")?;
    
    if !header.starts_with("Bearer ") {
        return Err("Invalid Authorization header format".to_string());
    }

    let token = &header[7..]; // Rimuove "Bearer "
    println!("Token: {}", token);
    
    let result = auth_service.validate_token(token);
    println!("Token validation result: {:?}", result);
    result
}

async fn login(
    State(auth_service): State<Arc<AuthService>>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    match auth_service.login(req) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (StatusCode::UNAUTHORIZED, e).into_response(),
    }
}

// handlers
async fn list_dir_with_empty_path(
    state: State<Arc<AuthService>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    list_dir(state, Path("".to_string()), headers).await
}

async fn list_dir(
    State(auth_service): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    println!("=== LIST_DIR CALLED === Path: '{}'", path);

    let username = match extract_user_from_headers(&headers, &auth_service) {
        Ok(user) => {
            println!("Authenticated user: {}", user);
            user
        },
        Err(e) => {
            println!("Authentication failed: {}", e);
            return (StatusCode::UNAUTHORIZED, e).into_response();
        },
    };

    // ✅ AGGIUNGI: Debug del filesystem reale
    let user_path = format!("remote-fs/{}", username);
    println!("🔍 Creating filesystem from path: {}", user_path);
    
    if let Ok(entries) = std::fs::read_dir(&user_path) {
        println!("📁 Real directory contents:");
        for entry in entries {
            if let Ok(entry) = entry {
                let file_name = entry.file_name();
                let file_type = if entry.path().is_dir() { "DIR" } else { "FILE" };
                println!("  - {} ({})", file_name.to_str().unwrap(), file_type);
            }
        }
    }

    let mut fs = match create_user_filesystem(&username) {
        Ok(fs) => fs,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    fs.change_dir("/").ok();
    
    // ✅ AGGIUNGI: Debug dello stato iniziale del filesystem virtuale
    println!("🖥️ Virtual filesystem root contents (before navigation): {:?}", fs.list_contents());

    let target_path = if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", path)
    };
    
    println!("🎯 Target path: '{}'", target_path);
    
    let res = if target_path == "/" {
        // Per la root, non facciamo change_dir aggiuntivi
        Ok(())
    } else {
        fs.change_dir(&target_path)
    };

    match res {
        Ok(_) => {
            let contents = fs.list_contents();
            println!("✅ Final virtual filesystem contents: {:?}", contents);
            Json(contents).into_response()
        },
        Err(e) if e.contains("not found") => (
            StatusCode::NOT_FOUND,
            e,
        ).into_response(),
        Err(e) if e.contains("Permission denied") => (
            StatusCode::FORBIDDEN,
            e,
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            e,
        ).into_response(),
    }
}

async fn read_file(
    State(auth_service): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // verify that the user is authenticated
    let username = match extract_user_from_headers(&headers, &auth_service) { // ← Passa auth_service
        Ok(user) => {
            println!("Authenticated user: {}", user);
            user
        },
        Err(e) => {
            println!("Authentication failed: {}", e);
            return (StatusCode::UNAUTHORIZED, e).into_response();
        },
    };

    let mut fs = match create_user_filesystem(&username) {
        Ok(fs) => fs,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    fs.change_dir("/").ok(); // return back to the root beafore performing any other call

    let result=fs.read_file(path.as_str());
    match result{
        Ok(content) => content.into_response(),
        Err(e) if e.contains("not found") => (
            StatusCode::NOT_FOUND,
            e,
        ).into_response(),
        Err(e) if e.contains("Invalid") => (
            StatusCode::BAD_REQUEST,
            e,
        ).into_response(),
        Err(e) if e.contains("Permission denied") => (
            StatusCode::FORBIDDEN,
            e,
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            e,
        ).into_response(),
    }
}

async fn write_file(
    State(auth_service): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {

    let username = match extract_user_from_headers(&headers, &auth_service) { // ← Passa auth_service
        Ok(user) => {
            println!("Authenticated user: {}", user);
            user
        },
        Err(e) => {
            println!("Authentication failed: {}", e);
            return (StatusCode::UNAUTHORIZED, e).into_response();
        },
    };

    let mut fs = match create_user_filesystem(&username) {
        Ok(fs) => fs,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    fs.change_dir("/").ok(); // return back to the root beafore performing any other call

    let result=fs.write_file(path.as_str(), &body);
    match result{
        Ok(_) => "File written successfully".into_response(),
        Err(e) if e.contains("not found") => (
            StatusCode::NOT_FOUND,
            e,
        ).into_response(),
        Err(e) if e.contains("Invalid") => (
            StatusCode::BAD_REQUEST,
            e,
        ).into_response(),
        Err(e) if e.contains("Permission denied") => (
            StatusCode::FORBIDDEN,
            e,
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            e,
        ).into_response(),
    }
}

async fn delete_file(
    State(auth_service): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {

    let username = match extract_user_from_headers(&headers, &auth_service) { // ← Passa auth_service
        Ok(user) => {
            println!("Authenticated user: {}", user);
            user
        },
        Err(e) => {
            println!("Authentication failed: {}", e);
            return (StatusCode::UNAUTHORIZED, e).into_response();
        },
    };

    let mut fs = match create_user_filesystem(&username) {
        Ok(fs) => fs,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    fs.change_dir("/").ok(); // return back to the root beafore performing any other call

    println!("{}", path);
    
    let result=fs.delete(path.as_str());

    match result{
        Ok(_) => "Directory/File deleted successfully".into_response(),
        Err(e) if e.contains("not found") => (
            StatusCode::NOT_FOUND,
            e,
        ).into_response(),
        Err(e) if e.contains("Permission denied") => (
            StatusCode::FORBIDDEN,
            e,
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            e,
        ).into_response(),
    }
}

async fn mkdir(
    State(auth_service): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Crea la directory

    let username = match extract_user_from_headers(&headers, &auth_service) { // ← Passa auth_service
        Ok(user) => {
            println!("Authenticated user: {}", user);
            user
        },
        Err(e) => {
            println!("Authentication failed: {}", e);
            return (StatusCode::UNAUTHORIZED, e).into_response();
        },
    };

    let mut fs = match create_user_filesystem(&username) {
        Ok(fs) => fs,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    fs.change_dir("/").ok(); // return back to the root beafore performing any other call

    let path = StdPath::new(&path);
    
    let old_dir=path.parent() // Ottieni il percorso senza l'ultima cartella
        .map(|p| p.to_str().unwrap_or("").to_string());// Converti in String

    let new_dir=path.file_name() // Ottieni il nome dell'ultima cartella
        .map(|f| f.to_str().unwrap_or("").to_string());// Converti in String

    let result=fs.make_dir(&format!("/{}", old_dir.unwrap()), &new_dir.unwrap());


    match result{
        Ok(_) => "Directory created successfully".into_response(),
        Err(e) if e.contains("not found") => (
            StatusCode::NOT_FOUND,
            e,
        ).into_response(),
        Err(e) if e.contains("Invalid") => (
            StatusCode::BAD_REQUEST,
            e,
        ).into_response(),
        Err(e) if e.contains("already exists") => (
            StatusCode::CONFLICT,
            e,
        ).into_response(),
        Err(e) if e.contains("Permission denied") => (
            StatusCode::FORBIDDEN,
            e,
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            e,
        ).into_response(),
    }
}
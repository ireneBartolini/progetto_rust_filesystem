use server::FileSystem;
mod auth;
use auth::{AuthService, LoginRequest, RegisterRequest};

use rusqlite::{params, Connection, Result as SqlResult};

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

#[derive(Clone)]
struct AppState {
    auth_service: Arc<AuthService>,
    filesystem: Arc<Mutex<Option<FileSystem>>>, // condiviso e clonabile
    connection: Arc< Mutex<Connection>>,
}

#[tokio::main]
async fn main()-> SqlResult<()> {
    // Crea (o apre) un database chiamato "mio_database.db"
    let connection  = Arc::new(Mutex::new(Connection::open("database/db.db")?));

    // creation of the auth service
    let auth_service = Arc::new(AuthService::new( connection.clone()  ));
    let fs = Arc::new(Mutex::new(None));

    let state = AppState {
        auth_service,
        filesystem: fs,
        connection,
    };

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
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("Server listening on {}", addr);
    
    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app.into_make_service(),
    )
    .await
    .unwrap();
    Ok(())
}

// function to create the file system
fn create_user_filesystem(username: &str) -> Result<FileSystem, String> {
    let user_path = format!("remote-fs/{}", username);
    let mut fs = FileSystem::from_file_system(&user_path);
    fs.set_side_effects(true);
    Ok(fs)
}

async fn register(
    State(app_state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let auth_service = &app_state.auth_service;
    match auth_service.register(req) {
        Ok(message) => {
            (StatusCode::CREATED, message).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

// FUNCTION TO EXTRACT A USER
fn extract_user_from_headers(headers: &HeaderMap, auth_service: &AuthService) -> Result<String, String> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    let header = auth_header.ok_or("Missing Authorization header")?;
    
    if !header.starts_with("Bearer ") {
        return Err("Invalid Authorization header format".to_string());
    }

    let token = &header[7..]; // Rimuove "Bearer "
    auth_service.validate_token(token)
}

async fn login(
    State(app_state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let auth_service = &app_state.auth_service;
    match auth_service.login(req) {
        Ok(response) => {
            if let Ok(new_fs) = create_user_filesystem(&response.username) {
                // Aggiorna il filesystem nell'AppState
                let mut fs = app_state.filesystem.lock().unwrap();
                *fs = Some(new_fs);
            }
            Json(response).into_response()
        },
        Err(e) => (StatusCode::UNAUTHORIZED, e).into_response(),
    }
}

// handlers
async fn list_dir_with_empty_path(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    list_dir(State(state), Path("".to_string()), headers).await
}

async fn list_dir(
    State(app_state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let auth_service = &app_state.auth_service;
    let username = match extract_user_from_headers(&headers, &auth_service) {
        Ok(user) => user,
        Err(e) => return (StatusCode::UNAUTHORIZED, e).into_response(),
    };

    let mut guard = app_state.filesystem.lock().unwrap();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "filesystem non inizializzato").into_response(),
    };

    let target_path = if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", path)
    };

    let res = if target_path == "/" {
        Ok(())
    } else {
        fs.change_dir(&target_path)
    };

    match res {
        Ok(_) => Json(fs.list_contents()).into_response(),
        Err(e) if e.contains("not found") => (StatusCode::NOT_FOUND, e).into_response(),
        Err(e) if e.contains("Permission denied") => (StatusCode::FORBIDDEN, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn read_file(
    State(app_state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let auth_service = &app_state.auth_service;
    if let Err(e) = extract_user_from_headers(&headers, &auth_service) {
        return (StatusCode::UNAUTHORIZED, e).into_response();
    }

    let mut guard = app_state.filesystem.lock().unwrap();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "filesystem non inizializzato").into_response(),
    };

    fs.change_dir("/").ok();
    match fs.read_file(&path) {
        Ok(content) => content.into_response(),
        Err(e) if e.contains("not found") => (StatusCode::NOT_FOUND, e).into_response(),
        Err(e) if e.contains("Invalid") => (StatusCode::BAD_REQUEST, e).into_response(),
        Err(e) if e.contains("Permission denied") => (StatusCode::FORBIDDEN, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn write_file(
    State(app_state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let auth_service = &app_state.auth_service;
    if let Err(e) = extract_user_from_headers(&headers, &auth_service) {
        return (StatusCode::UNAUTHORIZED, e).into_response();
    }

    let mut guard = app_state.filesystem.lock().unwrap();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "filesystem non inizializzato").into_response(),
    };

    fs.change_dir("/").ok();
    match fs.write_file(&path, &body) {
        Ok(_) => "File written successfully".into_response(),
        Err(e) if e.contains("not found") => (StatusCode::NOT_FOUND, e).into_response(),
        Err(e) if e.contains("Invalid") => (StatusCode::BAD_REQUEST, e).into_response(),
        Err(e) if e.contains("Permission denied") => (StatusCode::FORBIDDEN, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn delete_file(
    State(app_state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let auth_service = &app_state.auth_service;
    if let Err(e) = extract_user_from_headers(&headers, &auth_service) {
        return (StatusCode::UNAUTHORIZED, e).into_response();
    }

    let mut guard = app_state.filesystem.lock().unwrap();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "filesystem non inizializzato").into_response(),
    };

    fs.change_dir("/").ok();
    match fs.delete(&path) {
        Ok(_) => "Directory/File deleted successfully".into_response(),
        Err(e) if e.contains("not found") => (StatusCode::NOT_FOUND, e).into_response(),
        Err(e) if e.contains("Permission denied") => (StatusCode::FORBIDDEN, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn mkdir(
    State(app_state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let auth_service = &app_state.auth_service;
    if let Err(e) = extract_user_from_headers(&headers, &auth_service) {
        return (StatusCode::UNAUTHORIZED, e).into_response();
    }

    let mut guard = app_state.filesystem.lock().unwrap();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "filesystem non inizializzato").into_response(),
    };

    fs.change_dir("/").ok();

    let path = StdPath::new(&path);
    let old_dir = path.parent().and_then(|p| p.to_str()).unwrap_or("");
    let new_dir = path.file_name().and_then(|f| f.to_str()).unwrap_or("");

    match fs.make_dir(&format!("/{}", old_dir), new_dir) {
        Ok(_) => "Directory created successfully".into_response(),
        Err(e) if e.contains("not found") => (StatusCode::NOT_FOUND, e).into_response(),
        Err(e) if e.contains("Invalid") => (StatusCode::BAD_REQUEST, e).into_response(),
        Err(e) if e.contains("already exists") => (StatusCode::CONFLICT, e).into_response(),
        Err(e) if e.contains("Permission denied") => (StatusCode::FORBIDDEN, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

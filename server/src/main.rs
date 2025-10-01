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
fn extract_user_from_headers(headers: &HeaderMap) -> Result<String, String> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    auth::AuthService::extract_user_from_header(auth_header)
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

// Handlers (da implementare)
async fn list_dir(
    State(_): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Read the directory and return the list of files/directories in JSON

    // verify that the user is authenticated
    let username = match extract_user_from_headers(&headers) {
        Ok(user) => user,
        Err(e) => return (StatusCode::UNAUTHORIZED, e).into_response(),
    };

    // create the local filesystem
    let mut fs = match create_user_filesystem(&username) {
        Ok(fs) => fs,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    fs.change_dir("/").ok(); // return back to the root before performing any operation

    // go to the directory
    let res = fs.change_dir(&format!("/{}", path));

    match res{
        Ok(_) => Json(fs.list_contents()).into_response(),
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
    State(_): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // verify that the user is authenticated
    let username = match extract_user_from_headers(&headers) {
        Ok(user) => user,
        Err(e) => return (StatusCode::UNAUTHORIZED, e).into_response(),
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
    State(_): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {

    let username = match extract_user_from_headers(&headers) {
        Ok(user) => user,
        Err(e) => return (StatusCode::UNAUTHORIZED, e).into_response(),
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
    State(_): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {

    let username = match extract_user_from_headers(&headers) {
        Ok(user) => user,
        Err(e) => return (StatusCode::UNAUTHORIZED, e).into_response(),
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
    State(_): State<Arc<AuthService>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Crea la directory

    let username = match extract_user_from_headers(&headers) {
        Ok(user) => user,
        Err(e) => return (StatusCode::UNAUTHORIZED, e).into_response(),
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
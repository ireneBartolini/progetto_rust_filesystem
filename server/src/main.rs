use axum::{
    routing::{get, put, post, delete},
    Router, extract::Path, response::IntoResponse, Json,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/list/*path", get(list_dir))
        .route("/files/*path", get(read_file).put(write_file).delete(delete_file))
        .route("/mkdir/*path", post(mkdir));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("Server listening on {}", addr);
    
    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app.into_make_service(),
    )
    .await
    .unwrap();
}

// Handlers (da implementare)
async fn list_dir(Path(path): Path<String>) -> impl IntoResponse {
    // Leggi la directory e restituisci la lista di file/dir in JSON
    Json(vec!["file1.txt", "dir1"])
}

async fn read_file(Path(path): Path<String>) -> impl IntoResponse {
    // Leggi il file e restituisci i dati
    "contenuto del file"
}

async fn write_file(Path(path): Path<String>, body: String) -> impl IntoResponse {
    // Scrivi il file con il contenuto ricevuto
    "ok"
}

async fn delete_file(Path(path): Path<String>) -> impl IntoResponse {
    // Cancella file o directory
    "ok"
}

async fn mkdir(path: &str) -> impl IntoResponse {
    // Crea la directory

    let mut fs =  FileSystem::new();
    fs.set_side_effects(true);
    fs.set_real_path("remote-fs");

    let path = Path::new(path);
    
    let old_dir=path.parent() // Ottieni il percorso senza l'ultima cartella
        .map(|p| p.to_str().unwrap_or("").to_string());// Converti in String
    let new_dir=path.file_name() // Ottieni il nome dell'ultima cartella
        .map(|f| f.to_str().unwrap_or("").to_string());// Converti in String

    let result=fs.make_dir(old_dir, new_dir);
    match result {
        Ok(_) => "Directory created successfully",
        Err(e) => e.as_str(),
    }
}
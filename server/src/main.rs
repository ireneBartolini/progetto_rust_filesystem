use server::FileSystem;
use std::sync::{Arc, Mutex};
use std::path::Path as StdPath;
use axum::{
    routing::{get, put, post, delete},
    Router, extract::Path, extract::State, response::IntoResponse, Json,
    http::StatusCode,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // The access to the FileSystem is handled through a Mutex, in order to avoid concurrent accesses
    let fs = Arc::new(Mutex::new(FileSystem::from_file_system("remote-fs")));
    fs.lock().unwrap().set_side_effects(true);


    let app = Router::new()
        .route("/list/*path", get(list_dir))
        .route("/files/*path", get(read_file).put(write_file).delete(delete_file))
        .route("/mkdir/*path", post(mkdir))
        .with_state(fs.clone()); // The state is passed to the handlers;

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
async fn list_dir(
    State(fs): State<Arc<Mutex<FileSystem>>>,
    Path(path): Path<String>
) -> impl IntoResponse {
    // Read the directory and return the list of files/directories in JSON

    // Acquire the lock on the file system
    let mut fs = fs.lock().unwrap();

    println!("{}", path);

    // go to the directory
    let res = fs.change_dir(&path);

    match res{
        Ok(_) => Json(vec!["file1.txt".to_string(), "dir1".to_string()]).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(vec![e]),
        ).into_response(),
    }
}

async fn read_file(
    State(fs): State<Arc<Mutex<FileSystem>>>,
    Path(path): Path<String>
) -> impl IntoResponse {
    // Leggi il file e restituisci i dati
    "contenuto del file"
}

async fn write_file(
    State(fs): State<Arc<Mutex<FileSystem>>>,
    Path(path): Path<String>, body: String
) -> impl IntoResponse {
    // Scrivi il file con il contenuto ricevuto
    "ok"
}

async fn delete_file(
    State(fs): State<Arc<Mutex<FileSystem>>>,
    Path(path): Path<String>
) -> impl IntoResponse {
    // Cancella file o directory
    "ok"
}

async fn mkdir(
    State(fs): State<Arc<Mutex<FileSystem>>>,
    Path(path): Path<String>
) -> impl IntoResponse {
    // Crea la directory

    let mut fs = fs.lock().unwrap();

    let path = StdPath::new(&path);
    
    let old_dir=path.parent() // Ottieni il percorso senza l'ultima cartella
        .map(|p| p.to_str().unwrap_or("").to_string());// Converti in String

    let new_dir=path.file_name() // Ottieni il nome dell'ultima cartella
        .map(|f| f.to_str().unwrap_or("").to_string());// Converti in String

    let result=fs.make_dir(&format!("/{}", old_dir.unwrap()), &new_dir.unwrap());

    match result{
        Ok(_) => "Directory created successfully".into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            e,
        ).into_response(),
    }
}
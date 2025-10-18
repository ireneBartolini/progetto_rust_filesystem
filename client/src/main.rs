
use client::fuse_mod::RemoteFS;
use serde::{Deserialize, Serialize};
use tokio::task;

use std::{io::{self, Write}, sync::{atomic::{AtomicBool, Ordering}, Arc}, thread, time::Duration};
use rpassword::read_password;
use reqwest::Client;
use users::{get_user_by_name};
use std::process::Command;
use daemonize::Daemonize;
use std::fs::File;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::iterator::Signals;

#[derive(Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize, Debug)]
struct LoginResponse {
    token: String,
}



fn ensure_unmounted(mountpoint: &str) {
// let _ = fs::remove_dir_all(mountpoint);
// let _ = fs::create_dir_all(mountpoint);
    let status = Command::new("fusermount3")
        .arg("-u")
        .arg(mountpoint)
        .status();

    match status {
        Ok(s) if s.success() => println!("Unmounted existing mount at {}", mountpoint),
        Ok(_) => println!("Mount not mounted or already unmounted."),
        Err(e) => eprintln!("Error unmounting {}: {:?}", mountpoint, e),
    }
}

// funzione per assicurare che l'utente locale esista
fn ensure_local_user(username: &str) -> (u32, u32) {
    if let Some(user) = get_user_by_name(username) {
        // utente già esistente
        (user.uid(), user.primary_group_id())
    } else {
        println!("L'utente '{}' non esiste localmente, lo creo...", username);

        // Creazione utente locale tramite `useradd`
        // ATTENZIONE: richiede permessi sudo/root
        let status = Command::new("sudo")
            .arg("useradd")
            .arg("-m") // crea anche la home
            .arg(username)
            .status()
            .expect("Impossibile eseguire useradd");

        if !status.success() {
            panic!("Errore nella creazione dell'utente locale '{}'", username);
        }

        // Recupera i dati appena creati
        let user = get_user_by_name(username)
            .expect("Utente non trovato anche dopo la creazione!");

        (user.uid(), user.primary_group_id())
    }
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
   
    println!("== Remote FS ==");
     //login or registration
    let mut account= false;
     while !account{
        print!("Do you already have an account? (y/n)");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        let answer = answer.trim().to_uppercase().to_string();
        if  answer=="Y".to_string(){
                account=true;
        }else if answer=="N".to_string(){
                //Registratrion 
                println!("== Registration ==");
                    // Input username
                print!("Username: ");
                io::stdout().flush()?;
                let mut username = String::new();
                io::stdin().read_line(&mut username)?;
                let username = username.trim().to_string();

                // Input password (nascosta)
                print!("Password: ");
                io::stdout().flush()?;
                let password = read_password().unwrap();

                let client = Client::new();
                
                let res=  rt.block_on(async {
                    // richieste HTTP
                    let res = client.post("http://127.0.0.1:8080/auth/register")
                            .json(&LoginRequest { username, password })
                            .send()
                            .await;
                    res
                     });     
                

                match res {
                    Ok(r) if r.status().is_success()=>{
                        println!("✅ Correctly registered");
                        account = true; 
                    
                    }
                    _=> { return Err( "Error with registration".into());}
                }
        }
        
    }//fine loop registrazione
        

    // Input username
    println!("== Login ==");
    print!("Username: ");
    io::stdout().flush()?;
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim().to_string();

    // Input password (nascosta)
    print!("Password: ");
    io::stdout().flush()?;
    let password = read_password().unwrap();

    let current_user= username.clone();
    let client = Client::new();
        
    let login_res=  rt.block_on(async {
                    // richieste HTTP
                    let res = client.post("http://127.0.0.1:8080/auth/login")
                                                .json(&LoginRequest { username, password })
                                                .send()
                                                .await
                                                .map_err(|e| format!("HTTP request failed: {}", e))?;
                    
                    if res.status().is_success() {
                        let body: LoginResponse = res
                                                    .json()
                                                    .await
                                                    .map_err(|e| format!("Parsing JSON failed: {}", e))?;
                        Ok(body)
                    } else {
                        Err::<LoginResponse, String>(format!("Login failed: HTTP {}", res.status()))
                    }       
                }).map_err(Box::<dyn std::error::Error>::from)?;

   
            let token= login_res.token;
            println!("token: {}", token);

            // creao l'utente/restituisce uid e gid
            let (uid, gid) = ensure_local_user(&current_user);
            println!("Utente locale '{}' → UID={}, GID={}", current_user.clone(), uid, gid);
            

            let stdout = File::create("/tmp/myfs.out").unwrap();
            let stderr = File::create("/tmp/myfs.err").unwrap();

            let daemonize = Daemonize::new()
                .pid_file("/tmp/myfs.pid") // dove salvare il PID
                .chown_pid_file(true)
                .working_directory("/home/irene/progetto_rust_filesystem/client") // directory di lavoro
                .stdout(stdout)
                .stderr(stderr)
                .privileged_action(|| "Preparazione completata");

            match daemonize.start() {
                Ok(_) => {
                    println!("Daemon avviato correttamente, mount in corso...");
                    
                    // 🔥 Crea un nuovo runtime Tokio nel processo demone
                    let rt = tokio::runtime::Runtime::new()?;

                    // 🔥 Passalo alle operazioni FUSE
                    rt.block_on(async {
                        let fs = RemoteFS::new(
                        "http://127.0.0.1:8080".to_string(),
                        token,
                        uid,
                        gid,
                        );

                    // qui monto il filesystem come sempre
                    let mountpoint = "/home/irene/progetto_rust_filesystem/client/mount";
                    ensure_unmounted(mountpoint);
                    // 🔄 Flag condivisa per sapere quando terminare
                    let running = Arc::new(AtomicBool::new(true));
                    let r = running.clone();

                //thread che si occupa dello spegnimento/smontamento
                    thread::spawn(move || {
                        let mut signals = Signals::new(TERM_SIGNALS).unwrap();
                        if let Some(sig) = signals.forever().next() {
                            println!("📩 Ricevuto segnale {:?}, smonto FS...", sig);
                            r.store(false, Ordering::SeqCst);

                            // Smonta il filesystem
                            ensure_unmounted(mountpoint);
                            std::process::exit(0);
                        }
                    });  


                    println!("Mounting Remote FS at {}", mountpoint);
                    
                    if let Err(e)= fuser::mount2(fs, mountpoint, &[]){
                        eprintln!("Errore nel mount: {}", e);
                    }
                    //  Loop principale: il daemon resta attivo finché running = true
                    while running.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_secs(2));
                    }

                    println!("Uscita dal daemon, smontaggio...");
                    ensure_unmounted(mountpoint);
                    });
                }
                Err(e) => eprintln!("Errore nell'avvio del daemon: {}", e),
            } 
                
    Ok(())
}





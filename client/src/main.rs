
use client::fuse_mod::RemoteFS;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use rpassword::read_password;
use reqwest::Client;
use users::{get_user_by_name};
use std::process::Command;
use daemonize::Daemonize;

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



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
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
                let res = client.post("http://127.0.0.1:8080/auth/register")
                            .json(&LoginRequest { username, password })
                            .send()
                            .await?;
                        
                let status= res.status();       
                if status.is_success() {
                    println!("✅ Correctly registered");
                    account = true;
                } else {
                    let text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    println!("❌ Registration failed: HTTP {} - {}", status, text);
                }  
                
        }
        
    }
        

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
    let res = client.post("http://127.0.0.1:8080/auth/login")
        .json(&LoginRequest { username, password })
        .send()
        .await?;
        
    
    let status= res.status();       
    if status.is_success() {
        println!("✅ Success Login");
        let body= res.json();
        let login_res: LoginResponse = body.await?;
        let token= login_res.token;
        println!("token: {}", token);

        // creao l'utente/restituisce uid e gid
        let (uid, gid) = ensure_local_user(&current_user);
        println!("Utente locale '{}' → UID={}, GID={}", current_user.clone(), uid, gid);
        
        let fs = RemoteFS::new("http://127.0.0.1:8080".to_string(), token, uid, gid);
        let mountpoint = "/home/irene/progetto_rust_filesystem/client/mount";
        ensure_unmounted(mountpoint);
        println!("Mounting Remote FS at {}", mountpoint);
        
        fuser::mount2(fs, mountpoint, &[])?;   
    } else {
        let text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        println!("❌ Login failed: HTTP {} - {}", status, text);
        }
    
    
    
    
    Ok(())
   
}





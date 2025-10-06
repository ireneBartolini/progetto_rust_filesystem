
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use rpassword::read_password;

#[derive(Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize, Debug)]
struct LoginResponse {
    token: String,
}



use reqwest::Client;
use tokio;

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
                println!("Register:");
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
    let res = client.post("http://127.0.0.1:8080/auth/login")
        .json(&LoginRequest { username, password })
        .send()
        .await?;
        
    let status= res.status();       
    if status.is_success() {
        println!("✅ Success Login");
        let body= res.json();
        let login_res: LoginResponse = body.await?;
        println!("token: {}", login_res.token);
    } else {
        let text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        println!("❌ Login failed: HTTP {} - {}", status, text);
        }  
        
        Ok(())
   
}


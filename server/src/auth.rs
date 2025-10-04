use serde::{Deserialize, Serialize};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use bcrypt::{hash, verify, DEFAULT_COST};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{Duration, Utc};
use rusqlite::{Connection,  params, Result as SQLResult};

// Struttura per i claims del JWT
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // username
    pub exp: usize,         // expiration time
    pub iat: usize,         // issued at
}

// Struttura per l'utente
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
}

// Richiesta di login
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

// Richiesta di registrazione
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

// Risposta di autenticazione
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub username: String,
    pub expires_in: usize,
}

// Database utenti 
pub type UserDB = Arc<Mutex<HashMap<String, User>>>;

// Chiave segreta per firmare i JWT (in produzione usare variabile ambiente)
const JWT_SECRET: &str = "malnati-e-bello";

pub struct AuthService {
    users: UserDB,
    conn: Arc<Mutex<Connection>>
}

impl AuthService {
    pub fn new( conn: Arc<Mutex<Connection>>) -> Self {
        let u = AuthService::load_from_db(conn.clone());
        let users_map = u.unwrap_or_default();
        Self {
            users: Arc::new(Mutex::new(users_map)),
            conn: conn.clone()
        }
    }

    // Registra un nuovo utente
    pub fn register(&self, req: RegisterRequest) -> Result<String, String> {
        let mut users = self.users.lock().unwrap();
        
        // Controlla se l'utente esiste già
        if users.contains_key(&req.username) {
            return Err("Username already exists".to_string());
        }

        // Controlla che password sia valida
        if req.password.len() < 6 {
            return Err("Password must be at least 6 characters".to_string());
        }

        // Hash della password
        let password_hash = hash(&req.password, DEFAULT_COST)
            .map_err(|_| "Failed to hash password")?;

        // Crea l'utente
        let user = User {
            username: req.username.clone(),
            password_hash,
        };

        

        users.insert(req.username.clone(), user.clone());
        //lo aggiunge nel db 
        let _=self.save_to_db(user);
        Ok("User registered successfully".to_string())
    }

    // Login utente
    pub fn login(&self, req: LoginRequest) -> Result<AuthResponse, String> {

        let users = self.users.lock().unwrap();
        
        // Trova l'utente
        let user = users.get(&req.username)
            .ok_or("Invalid username or password")?;

        // Verifica la password
        let is_valid = verify(&req.password, &user.password_hash)
            .map_err(|_| "Authentication failed")?;

        if !is_valid {
            return Err("Invalid username or password".to_string());
        }

        // ensure there is a user directory
        self.ensure_user_directory(&req.username)?;

        // Genera JWT token
        let token = self.generate_token(&req.username)?;
        
        Ok(AuthResponse {
            token,
            username: req.username,
            expires_in: 3600, // 1 ora
        })
    }

    // function to create the user directory
    fn ensure_user_directory(&self, username: &str) -> Result<(), String> {
        use std::fs;
        let user_dir = format!("remote-fs/{}", username);
        
        if !std::path::Path::new(&user_dir).exists() {
            fs::create_dir_all(&user_dir)
                .map_err(|e| format!("Failed to create user directory: {}", e))?;
            //println!("Created directory for user: {}", username);
        } else {
            // Controlla e rimuovi directory annidate problematiche
            let nested_dir = format!("{}/{}", user_dir, username);
            if std::path::Path::new(&nested_dir).exists() {
                println!("Found problematic nested directory {}, removing it", nested_dir);
                fs::remove_dir_all(&nested_dir)
                    .map_err(|e| format!("Failed to remove nested directory: {}", e))?;
                println!("Removed nested directory successfully");
            }
        }
        
        Ok(())
    }

    // Genera JWT token
    fn generate_token(&self, username: &str) -> Result<String, String> {
        let expiration = Utc::now()
            .checked_add_signed(Duration::hours(1))
            .expect("valid timestamp")
            .timestamp() as usize;

        let claims = Claims {
            sub: username.to_string(),
            exp: expiration,
            iat: Utc::now().timestamp() as usize,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(JWT_SECRET.as_ref()),
        )
        .map_err(|_| "Failed to generate token".to_string())
    }

    // Valida JWT token
    pub fn validate_token(&self, token: &str) -> Result<String, String> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(JWT_SECRET.as_ref()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|_| "Invalid token".to_string())?;

        Ok(token_data.claims.sub)
    }

    // Salva utenti su DB 
    pub fn save_to_db(&self, user: User) -> SQLResult<()> {
        let conn= self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO USER (Username, Password) VALUES (?1, ?2) ", 
            params![user.username, user.password_hash],
        )?;
        Ok(())

    }

    // Carica utenti da file
    pub fn load_from_db( conn: Arc<Mutex<Connection>>) -> Result<HashMap<String, User> , String> {
       
       // Leggi i dati
    let c= conn.lock().unwrap();
    let stmt = c.prepare("SELECT username, password FROM user");
    match stmt {
        Ok(mut statement) => {
            let mut user_map = HashMap::new();
            let user_iter = statement.query_map([], |row| {
                let username: String = row.get(0)?;
                let password_hash: String = row.get(1)?;
                Ok(User {
                    username,
                    password_hash,
                })
            }).map_err(|e| e.to_string())?;

            for user_result in user_iter {
                let user = user_result.map_err(|e| e.to_string())?;
                user_map.insert(user.username.clone(), user);
            }
            Ok(user_map)
        },
        Err(_) => {
            Err("Non esiste una tabella USER".to_string())
        }
        }
    
    }
}
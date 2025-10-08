
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use rpassword::read_password;
use libc::ENOENT;
use reqwest::Client;


#[derive(Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize, Debug)]
struct LoginResponse {
    token: String,
}

#[derive(Serialize)]
struct WriteRequest<'a> {
    path: String,
    data: &'a [u8],
}

struct RemoteFS {
    base_url: String,
    token: String,
}

impl RemoteFS {
    fn new(base_url: String, token: String) -> Self {
        Self {
            base_url,
            token,
        }
    }

    fn ino_to_path(&self, ino: u64) -> String {
        // Semplice mappatura demo: inode -> /file_ino
        format!("/file_{}", ino)
    }
}

use tokio::task;

use fuser::{FileAttr, FileType, Filesystem, Request, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyWrite};
use std::time::{Duration, SystemTime};
use std::ffi::OsStr;

impl Filesystem for RemoteFS {
    
    //create dummy: è necessaria per FUSe ma non chiama nessuna API
    fn create(
        &mut self,
        _req: &fuser::Request<'_>,
        _parent: u64,
        name: &std::ffi::OsStr,
        _mode: u32,
        _size: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        println!("CREATE called for {:?}", name);

        let attr = FileAttr {
            ino: 2, // finto inode
            size: 0,
            blocks: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: SystemTime::now(),
            kind: FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: 1000,
            gid: 1000,
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        // Non crea davvero nulla, ma fa contento il kernel
        reply.created(&Duration::new(1, 0), &attr, 0, 0, 0);
    }


    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let path = self.ino_to_path(ino);
        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();

task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .get(format!("{}/files", base_url))
                    .bearer_auth(token)
                    .query(&[("path", &path)])
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => {
                        let content = r.bytes().await.unwrap_or_default();
                        let start = offset as usize;
                        let end = (offset as usize + size as usize).min(content.len());
                        reply.data(&content[start..end]);
                    }
                    _ => reply.error(ENOENT),
                }
            });
        });
    }

    fn write(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        _offset: i64,
        data: &[u8],
        _: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        let path = self.ino_to_path(ino);
        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();
        let data_copy = data.to_vec();

        task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .put(format!("{}/files", base_url))
                    .bearer_auth(token)
                    .json(&WriteRequest { path, data: &data_copy })
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => reply.written(data_copy.len() as u32),
                    _ => reply.error(ENOENT),
                }
            });
        });
    }

    fn getattr(&mut self, _: &Request, ino: u64, _: Option<u64>, reply: ReplyAttr) {
        println!("getattr(ino={})", ino);
        let ts = SystemTime::now();
        let attr = FileAttr {
            ino,
            size: 0,
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: 1000,
            gid: 1000,
            rdev: 0,
            flags: 0,
            blksize: 512,
        };
        reply.attr(&Duration::new(1, 0), &attr);
    }

    fn readdir(&mut self, _: &Request, ino: u64, _: u64, offset: i64, mut reply: ReplyDirectory) {
        println!("readdir(ino={}, offset={})", ino, offset);
        if ino == 1 {
            if offset == 0 {
                reply.add(1, 0, FileType::Directory, &OsStr::new("."));
                reply.add(1, 1, FileType::Directory, &OsStr::new(".."));
            }
            reply.ok();
        } else {
            reply.error(libc::ENOENT);
        }
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
        let fs = RemoteFS::new("http://127.0.0.1:8080".to_string(), token);
        let mountpoint = "/home/irene/progetto_rust_filesystem/client/mnt/remote-fs";
        println!("Mounting Remote FS at {}", mountpoint);
        fuser::mount2(fs, mountpoint, &[])?;   
    } else {
        let text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        println!("❌ Login failed: HTTP {} - {}", status, text);
        }
    
    
    
    
    Ok(())
   
}

use std::process::Command;

impl Drop for RemoteFS {
    fn drop(&mut self) {
        println!("smonto fuse");
        let _ = Command::new("fusermount3")
            .arg("-u")
            .arg("/home/irene/progetto_rust_filesystem/client/mnt/remote-fs")
            .status();
    }
}



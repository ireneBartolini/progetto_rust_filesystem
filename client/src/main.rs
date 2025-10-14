
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use rpassword::read_password;
use libc::ENOENT;
use reqwest::Client;
use std::collections::HashMap;
use libc::ENOSYS;
use libc::EIO;

#[derive(Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize, Debug)]
struct LoginResponse {
    token: String,
}

// #[derive(Serialize)]
// struct WriteRequest<'a> {
//     path: String,
//     data: &'a [u8],
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub permissions: String,        // es: "drwxr-xr-x", "-rw-r--r--"
    pub links: u32,                 // always 1
    pub owner: String,              // owner username
    pub group: String,              // group (always users)
    pub size: u64,                  // dimension in bytes
    pub modified: String,           // last modifiied date
    pub name: String,               // name of the file/directory
    pub is_directory: bool,         // flag to identify wether it is a directory or not
}

struct RemoteFS {
    base_url: String,
    token: String,
    inode_to_path: HashMap<u64, String>,
    path_to_parent: HashMap<String, u64>,
    next_ino: u64,

}

impl RemoteFS {
    fn new(base_url: String, token: String) -> Self {
        let mut map = HashMap::new();
        // La root (ino = 1)
        map.insert(1, "".to_string());
        let mut map_parent = HashMap::new();
        Self {
            base_url,
            token,
            inode_to_path: map,
            path_to_parent: map_parent,
            next_ino: 2,
        }
    }
    
    fn register_path(&mut self, path: &str) -> u64 {
        if let Some((&ino, _)) = self.inode_to_path.iter().find(|(_, p)| p.as_str() == path) {
            return ino;
        }
        let ino = self.next_ino;
        self.next_ino += 1;
        self.inode_to_path.insert(ino, path.to_string());

        // registra il parent
        if let Some(parent_path) = path.rsplit_once('/') {
            let parent = parent_path.0;
            if let Some((&parent_ino, _)) = self.inode_to_path.iter().find(|(_, p)| p.as_str() == parent) {
                self.path_to_parent.insert(path.to_string(), parent_ino);
             }
        }
        ino
    }

    fn get_path(&self, ino: u64) -> Option<String> {
        self.inode_to_path.get(&ino).cloned()
    }

    fn exist_path(&mut self, path: &str)-> Option<u64>{
        if let Some((&ino, _)) = self.inode_to_path.iter().find(|(_, p)| p.as_str() == path) {
            return Some(ino);
        }else{
            return None;
        }
    }

    fn join_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{}", name)
    } else {
        format!("{}/{}", parent, name)
    }
    }
    
}

use tokio::task;

use fuser::{FileAttr, FileType, Filesystem, Request, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyWrite, ReplyEntry, ReplyEmpty, ReplyOpen};
use std::time::{Duration, SystemTime};
use std::ffi::OsStr;

impl Filesystem for RemoteFS {
    fn mkdir(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        println!("mkdir(parent={}, name={:?})", parent, name);

        // Ricava il path logico della nuova directory
        let Some(parent_path) = self.get_path(parent) else {
        reply.error(ENOENT);
        return;
        };

        let dir_name = name.to_str().unwrap_or("");
        let full_path = if parent_path == "/" {
            format!("/{}", dir_name)
         } else {
            format!("{}/{}", parent_path, dir_name)
        };

        // Chiamata remota al server (esempio)
        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();

        let success = task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .post(format!("{}/mkdir/{}", base_url, full_path))
                    .bearer_auth(token)
                    .send()
                    .await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
            })
        });

        if !success {
            reply.error(EIO); // errore generico
            return;
        }

        // Se la creazione remota è andata bene, aggiorna la mappa inode↔path
        let ino=self.register_path(&full_path);

        // Costruisci gli attributi fittizi per la risposta
        let ts = SystemTime::now();
        let attr = FileAttr {
            ino,
            size: 0,
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: 1000,
            gid: 1000,
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        reply.entry(&Duration::new(1, 0), &attr, 0);
    }



   // create dummy: è necessaria per FUSe ma non chiama nessuna API
    fn create(
        &mut self,
        _req: &fuser::Request<'_>,
        parent: u64,
        name: &std::ffi::OsStr,
        _mode: u32,
        _size: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        println!("CREATE called for {:?}", name);
        let parent_path= self.get_path(parent).unwrap();
        let real_path= parent_path.to_owned()+"/"+name.to_str().unwrap();
        let ino= self.register_path(&real_path);
        
        let attr = FileAttr {
            ino, // finto inode
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
        let path = self.get_path(ino).unwrap();
        println!("execute read {}", path);
        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();

        task::block_in_place(|| {
            
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .get(format!("{}/files/{}", base_url, path))
                    .bearer_auth(token)
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
        
        let path = self.get_path(ino).unwrap();
        println!("execute write {}", path);
        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();
        let data_copy = data.to_vec();
        let body = String::from_utf8_lossy(data).to_string();

        task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .put(format!("{}/files/{}", base_url, path))
                    .bearer_auth(token)
                    .body(body)
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
        
        let path= self.get_path(ino).unwrap();
        println!("getattr(ino={}, path={})", ino, path);

        if ino==1{
            let ts = SystemTime::now();
            let attr = FileAttr {
                ino,
                size: 0,
                blocks: 1,
                atime: ts,
                mtime: ts,
                ctime: ts,
                crtime: ts,
                kind: FileType::Directory,
                perm:  0o755,
                nlink: 1,
                uid: 1000,
                gid: 1000,
                rdev: 0,
                flags: 0,
                blksize: 512,
                };
                reply.attr(&Duration::new(1, 0), &attr);

        }else{

        //API CALL
        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();
        task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .get(format!("{}/lookup/{}", base_url, path)) // path già con /
                    .bearer_auth(token)
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => {
                        println!("risposta corretta");
                        match r.json::<FileInfo>().await {
                            Ok(obj) => {
                                println!("json {:?}", obj);

                                let (kind, perm) = if obj.is_directory {
                                    (FileType::Directory, 0o755)
                                } else {
                                   (FileType::RegularFile, 0o644)
                                };

                                let ino = self.register_path(&path);

                                let ts = SystemTime::now();
                                let attr = FileAttr {
                                    ino,
                                    size: obj.size,
                                    blocks: 1,
                                    atime: ts,
                                    mtime: ts,
                                    ctime: ts,
                                    crtime: ts,
                                    kind,
                                    perm,
                                    nlink: 1,
                                    uid: 1000,
                                    gid: 1000,
                                    rdev: 0,
                                    flags: 0,
                                    blksize: 512,
                                };
                                reply.attr(&Duration::new(1, 0), &attr);
                            }
                            Err(_) => reply.error(ENOENT),
                        }
                }
                _ => reply.error(ENOENT),
            }
                
            })
        });
    }
    
    }


   

    fn readdir(
        &mut self, 
        _req: &Request, 
        ino: u64, _: u64, 
        offset: i64, 
        mut reply: 
        ReplyDirectory) {

        let path = match self.get_path(ino) {
            Some(p) => p.clone(),
            None => {
                reply.error(ENOENT);
                return;
            }
        };    
        println!("readdir(ino={}, offset={}, path={})", ino, offset, path);

    
        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();

        let files: Vec<FileInfo> =task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .get(format!("{}/list/{}", base_url, path)) // path già con /
                    .bearer_auth(token)
                    .send()
                    .await;

                    match resp {
                    Ok(r) if r.status().is_success() =>{
                        let v= r.json::<Vec<FileInfo>>().await;
                        let res;
                        //DEbug!
                        match v{
                            Ok(obj)=>{
                                //println!("json {:?}", obj); 
                                res=obj;
                            },
                            Err(_)=>{res=Vec::new();}
                        }
                        res                       
                    }
                    ,
                    _ => Vec::new(),
                    }
                
            })
        });

    let mut i = offset;

        if i == 0 {
            let current_ino = ino;
            let _ = reply.add(current_ino, 1, FileType::Directory, ".");

            // trova il parent
            let parent_ino = if current_ino == 1 {
                1 // root: parent == self
            } else {
                let path = self.get_path(current_ino).unwrap();
                *self.path_to_parent.get(&path).unwrap_or(&1)
            };

            let _ = reply.add(parent_ino, 2, FileType::Directory, "..");
        }


        for (idx, item) in files.iter().enumerate().skip((i - 2) as usize) {
            let name= item.name.clone();
            let kind;
            if item.is_directory{
                kind=FileType::Directory;
            }else{
                kind=FileType::RegularFile;
            }
            let next_offset = (idx as i64) + 3; // offset successivo
            let full_path = format!("{}/{}", path, name);
            let _= self.register_path(&full_path);
            
            let _ =reply.add((idx as u64) + 2, next_offset, kind, OsStr::new(&name));
        }

        reply.ok();
    
    }

//controlla che file/dir esitano o meno
    fn lookup(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        reply: ReplyEntry,
    ) {
    
        let parent_path= self.get_path(parent).unwrap();
        let path = if parent_path == "/" {
            format!("/{}", name.to_str().unwrap())
        } else {
            format!("{}/{}", parent_path, name.to_str().unwrap())
        };

            // ignora lookup spurie (es: echo, total, ecc.)
        if !name.to_str().unwrap().contains('.') && !name.to_str().unwrap().starts_with("child") {
           // println!("Ignoro lookup spurio su {:?}", name);
            reply.error(ENOENT);
            return;
        }

        println!("lookup(parent={}, name={:?})", parent, name);
      
        //API CALL
        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();
        let res: Option<FileInfo>= task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .get(format!("{}/lookup/{}", base_url, path)) // path già con /
                    .bearer_auth(token)
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => r.json::<FileInfo>().await.ok(),
                    _ => None,
                }
            })
        });
    
    // ora rispondi fuori dal contesto async
    match res {
        Some(obj) => {
           // println!("json {:?}", obj);

            let (kind, perm) = if obj.is_directory {
                (FileType::Directory, 0o755)
            } else {
                (FileType::RegularFile, 0o644)
            };

            let ino = self.register_path(&path);
            let ts = SystemTime::now();
            let attr = FileAttr {
                ino,
                size: obj.size,
                blocks: 1,
                atime: ts,
                mtime: ts,
                ctime: ts,
                crtime: ts,
                kind,
                perm,
                nlink: 1,
                uid: 1000,
                gid: 1000,
                rdev: 0,
                flags: 0,
                blksize: 512,
            };

            reply.entry(&Duration::new(10, 0), &attr, 0);
        }
        None => {
            println!("lookup fallita per {}", path);
            reply.error(ENOENT);
        }
    }

    
    }

//DUMMY FUNCTION FOR FUSE
    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        println!("open(ino={})", ino);
        if flags & libc::O_WRONLY != 0 || flags & libc::O_RDWR != 0 {
        println!("--> opening file for write");
    }
    reply.opened(0, 0); // handle fittizio = 0, flags = 0
    }

    // fn flush(&mut self, _req: &Request, ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
    //     println!("flush(ino={})", ino);
    //     reply.ok(); // non serve fare nulla
    // }

    // fn release(&mut self, _req: &Request, ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
    //     println!("release(ino={})", ino);

    //     reply.ok(); // idem
    // }

    
    fn unlink(
        &mut self, 
        _req: &Request, 
        parent: u64, 
        name: &OsStr, 
        reply: ReplyEmpty) 
    {
        println!("unlink(parent={}, name={:?})", parent, name);

        let Some(parent_path) = self.get_path(parent) else {
        reply.error(ENOENT);
        return;
        };

        let full_path = format!("{}/{}", parent_path, name.to_str().unwrap());
        println!("Deleting {}", full_path);

        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();

         task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .delete(format!("{}/files/{}", base_url, full_path))
                    .bearer_auth(token)
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => reply.ok(),
                    _ => reply.error(EIO),
                }
            });
        });
        
    }

    fn rmdir(
    &mut self,
    _req: &Request<'_>,
    parent: u64,
    name: &OsStr,
    reply: ReplyEmpty,) 
    {
        println!("rmdir(parent={}, name={:?})", parent, name);

        let Some(parent_path) = self.get_path(parent) else {
            reply.error(ENOENT);
            return;
        };

        let full_path = format!("{}/{}", parent_path, name.to_str().unwrap());
        println!("Removing directory {}", full_path);

        let client = Client::new();
        let token = self.token.clone();
        let base_url = self.base_url.clone();

        task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .delete(format!("{}/files/{}", base_url, full_path))
                    .bearer_auth(token)
                    .send()
                    .await;

                match resp {
                    Ok(r) if r.status().is_success() => reply.ok(),
                    _ => reply.error(EIO),
                }
            });
        });
    }


    

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

use std::process::Command;

impl Drop for RemoteFS {
    fn drop(&mut self) {
        println!("smonto fuse");
        let _ = Command::new("fusermount3")
            .arg("-u")
            .arg("/home/irene/progetto_rust_filesystem/client/mount")
            .status();
    }
}



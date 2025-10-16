pub mod fuse_mod{

use serde::{Deserialize, Serialize};
use libc::ENOENT;
use reqwest::Client;
use std::collections::HashMap;
use std::process::Command;
use libc::EIO;
use chrono::{DateTime};
use tokio::task;
use fuser::{FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite, Request};
use std::time::{Duration, SystemTime};
use std::ffi::OsStr;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub permissions: u16,       
    pub links: u32,                 // always 1
    pub owner: String,              // owner username
    pub group: String,              // group (always users)
    pub size: u64,                  // dimension in bytes
    pub modified: String,           // last modifiied date
    pub name: String,               // name of the file/directory
    pub is_directory: bool,         // flag to identify wether it is a directory or not
}

fn parse_time(s: &str) -> SystemTime {
    match DateTime::parse_from_rfc3339(s) {
        Ok(dt) => SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(dt.timestamp() as u64),
        Err(_) => SystemTime::now(),
    }
}

pub struct RemoteFS {
    base_url: String,
    token: String,
    inode_to_path: HashMap<u64, String>,
    path_to_parent: HashMap<String, u64>,
    next_ino: u64,
    uid: u32,
    gid: u32,
}

impl RemoteFS {
    pub fn new(base_url: String, token: String, uid: u32, gid: u32) -> Self {
        let mut map = HashMap::new();
        // La root (ino = 1)
        map.insert(1, "".to_string());
        let map_parent = HashMap::new();
        Self {
            base_url,
            token,
            inode_to_path: map,
            path_to_parent: map_parent,
            next_ino: 2,
            uid,
            gid
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
            Some(ino)
        }else{
            None
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
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        reply.entry(&Duration::new(1, 0), &attr, 0);
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
                uid: self.uid,
                gid: self.gid,
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

                                let kind = if obj.is_directory {
                                    FileType::Directory
                                } else {
                                   FileType::RegularFile
                                };

                                let ino = self.register_path(&path);

                                let ts = parse_time(&obj.modified);
                                let attr = FileAttr {
                                    ino,
                                    size: obj.size,
                                    blocks: (obj.size / 512).max(1),
                                    atime: ts,
                                    mtime: ts,
                                    ctime: ts,
                                    crtime: ts,
                                    kind,
                                    perm: obj.permissions,
                                    nlink: obj.links,
                                    uid: self.uid,
                                    gid: self.gid,
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
                        v.unwrap_or(Vec::new())                   
                    },
                    _ => Vec::new(),
                    }
                
            })
        });

    let i = offset;

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

        let name_str = name.to_str().unwrap_or("");

        // escludi le lookup spurie di comandi tipo "echo", "total", "drwxr-xr-x", numeri, ecc.
        //é 
        let is_spurious = name_str.chars().all(|c| c.is_numeric())
            || name_str.starts_with("drwx")
            || name_str.eq_ignore_ascii_case("total")
            || name_str.eq_ignore_ascii_case("echo")
            || name_str.eq_ignore_ascii_case("cat")
            || name_str.eq_ignore_ascii_case("ls")
            || name_str.eq_ignore_ascii_case("mkdir")
            || name_str.eq_ignore_ascii_case("rmdir");

        if is_spurious {
            println!("Ignoro lookup spurio su {:?}", name_str);
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

    match res {
        Some(obj) => {
            println!("json {:?}", obj);

            let kind= if obj.is_directory {
                FileType::Directory
            } else {
                FileType::RegularFile
            };

            let ino = self.register_path(&path);
            let ts = parse_time(&obj.modified);
            let attr = FileAttr {
                ino,
                size: obj.size,
                blocks: (obj.size / 512).max(1),
                atime: ts,
                mtime: ts,
                ctime: ts,
                crtime: ts,
                kind,
                perm: obj.permissions,
                nlink: obj.links,
                uid: self.uid,
                gid: self.gid,
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
        let ts=SystemTime::now();
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
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        // Non crea davvero nulla, ma fa contento il kernel
        reply.created(&Duration::new(1, 0), &attr, 0, 0, 0);
    }

//DUMMY FUNCTION FOR FUSE
    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        println!("open(ino={})", ino);
        if flags & libc::O_WRONLY != 0 || flags & libc::O_RDWR != 0 {
        println!("--> opening file for write");
       
        } 
        println!("open flags: 0o{:o}", flags);

    reply.opened(0, 0); // handle fittizio = 0, flags = 0
    }

    fn setattr(
    &mut self,
    _req: &Request<'_>,
    ino: u64,
    _mode: Option<u32>,
    _uid: Option<u32>,
    _gid: Option<u32>,
    size: Option<u64>,
    _atime: Option<fuser::TimeOrNow>,
    _mtime: Option<fuser::TimeOrNow>,
    _ctime: Option<SystemTime>,
    _fh: Option<u64>,
    _crtime: Option<SystemTime>,
    _chgtime: Option<SystemTime>,
    _bkuptime: Option<SystemTime>,
    _flags: Option<u32>,
    reply: ReplyAttr,
    ) {
        println!("setattr(ino={}, size={:?})", ino, size);
    // Se viene richiesta una truncation, gestiscila (es. manda una chiamata al server)
        if let Some(_new_size) = size {
        // qui puoi chiamare l'API remota per troncare il file, oppure accettare e rispondere localmente
        // per ora rispondiamo con attributi aggiornati (dummy)
        
        }

        let ts = SystemTime::now();
        let attr = FileAttr {
            ino,
            size: size.unwrap_or(0),
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            flags: 0,
            blksize: 512,
        };
        reply.attr(&Duration::new(1,0), &attr);
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
        let body = String::from_utf8_lossy(data).to_string();

        let ok: bool = task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let resp = client
                    .put(format!("{}/files/{}", base_url, path))
                    .bearer_auth(token)
                    .body(body)
                    .send()
                    .await;

                match resp {
                    Ok(r) => r.status().is_success(),
                    Err(_) => false,
                }
            })
        });

        if ok{
            reply.written(data.len() as u32);
        }else{
            reply.error(EIO);
        }
    }

    fn flush(&mut self, _req: &Request, ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        println!("flush(ino={})", ino);
        reply.ok(); // non serve fare nulla
    }

    fn fsync(&mut self, _req: &Request<'_>, ino: u64, fh: u64, datasync: bool, reply: ReplyEmpty) {
        println!("fsync(ino={}, fh={}, datasync={})", ino, fh, datasync);
        reply.ok();
    }

    fn release(&mut self, _req: &Request, ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        println!("release(ino={})", ino);

        reply.ok(); // idem
    }

    
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

impl Drop for RemoteFS {
    fn drop(&mut self) {
        println!("smonto fuse");
        let _ = Command::new("fusermount3")
            .arg("-u")
            .arg("/home/irene/progetto_rust_filesystem/client/mount")
            .status();
    }
}

}
pub mod fuse_mod{

use serde::{Deserialize, Serialize};
use libc::ENOENT;
use reqwest::blocking::Client as BlockingClient; // <--- aggiunto
use reqwest::Client;
use tokio::runtime::Runtime;
use std::collections::HashMap;
use std::process::Command;
use std::str::Bytes;
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
    write_buffer: Vec<u8>,
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
            gid,
            write_buffer: Vec::new(),
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

    // fn exist_path(&mut self, path: &str)-> Option<u64>{
    //     if let Some((&ino, _)) = self.inode_to_path.iter().find(|(_, p)| p.as_str() == path) {
    //         Some(ino)
    //     }else{
    //         None
    //     }
    // }

    // fn join_path(parent: &str, name: &str) -> String {
    // if parent == "/" {
    //     format!("/{}", name)
    // } else {
    //     format!("{}/{}", parent, name)
    // }
    // }
    
}

// ...existing code...
impl Filesystem for RemoteFS {
    // ...existing code...

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
        let path = match self.get_path(ino) {
            Some(p) => p,
            None => { reply.error(ENOENT); return; }
        };
        println!("execute read {} offset={} size={}", path, offset, size);

        let client = BlockingClient::new();
        let token = self.token.clone();
        let base_url = self.base_url.trim_end_matches('/').to_string();
        let url = format!("{}/files/{}", base_url, path);

        match client.get(&url).bearer_auth(token).send() {
            Ok(r) if r.status().is_success() => {
                match r.bytes() {
                    Ok(bytes) => {
                        let start = offset.max(0) as usize;
                        let end = std::cmp::min(start + size as usize, bytes.len());
                        if start >= bytes.len() {
                            reply.data(&[]);
                        } else {
                            reply.data(&bytes[start..end]);
                        }
                    }
                    Err(_) => reply.error(ENOENT),
                }
            }
            Ok(r) => {
                println!("GET {} returned HTTP {}", url, r.status());
                reply.error(ENOENT)
            }
            Err(e) => {
                println!("HTTP GET error {}: {}", url, e);
                reply.error(ENOENT)
            }
        }
    }

    fn getattr(&mut self, _: &Request, ino: u64, _: Option<u64>, reply: ReplyAttr) {
        let path = self.get_path(ino).unwrap_or_default();
        println!("getattr(ino={}, path={})", ino, path);

        if ino == 1 {
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
                perm: 0o755,
                nlink: 1,
                uid: self.uid,
                gid: self.gid,
                rdev: 0,
                flags: 0,
                blksize: 512,
            };
            reply.attr(&Duration::new(180, 0), &attr);
            return;
        }

        // uso client sincrono per metadata
        let client = BlockingClient::new();
        let token = self.token.clone();
        let base_url = self.base_url.trim_end_matches('/').to_string();
        let url = format!("{}/lookup/{}", base_url, path);

        match client.get(&url).bearer_auth(token).send() {
            Ok(r) if r.status().is_success() => match r.json::<FileInfo>() {
                Ok(obj) => {
                    println!("json {:?}", obj);
                    let kind = if obj.is_directory { FileType::Directory } else { FileType::RegularFile };
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
            },
            Ok(r) => {
                println!("GET meta {} returned HTTP {}", url, r.status());
                reply.error(ENOENT)
            }
            Err(e) => {
                println!("HTTP GET meta error {}: {}", url, e);
                reply.error(ENOENT)
            }
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let path = match self.get_path(ino) {
            Some(p) => p.clone(),
            None => { reply.error(ENOENT); return; }
        };
        println!("readdir(ino={}, offset={}, path={})", ino, offset, path);

        let client = BlockingClient::new();
        let token = self.token.clone();
        let base_url = self.base_url.trim_end_matches('/').to_string();
        let url = format!("{}/list/{}", base_url, path);

        let files: Vec<FileInfo> = match client.get(&url).bearer_auth(token).send() {
            Ok(r) if r.status().is_success() => r.json::<Vec<FileInfo>>().unwrap_or_default(),
            Ok(_) => Vec::new(),
            Err(e) => {
                println!("HTTP list error {}: {}", url, e);
                Vec::new()
            }
        };

        let i = offset;
        if i == 0 {
            let current_ino = ino;
            let _ = reply.add(current_ino, 1, FileType::Directory, ".");
            let parent_ino = if current_ino == 1 {
                1
            } else {
                let p = self.get_path(current_ino).unwrap_or("/".to_string());
                *self.path_to_parent.get(&p).unwrap_or(&1)
            };
            let _ = reply.add(parent_ino, 2, FileType::Directory, "..");
        }

        for (idx, item) in files.iter().enumerate().skip((i - 2) as usize) {
            let name = item.name.clone();
            let kind = if item.is_directory { FileType::Directory } else { FileType::RegularFile };
            let next_offset = (idx as i64) + 3;
            let full_path = if path == "/" { format!("/{}", name) } else { format!("{}/{}", path, name) };
            let ino = self.register_path(&full_path);
            let _ = reply.add(ino, next_offset, kind, OsStr::new(&name));
        }

        reply.ok();
    }

    fn lookup(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        reply: ReplyEntry,
    ) {
        let parent_path = self.get_path(parent).unwrap();
        let path = if parent_path == "/" {
            format!("/{}", name.to_str().unwrap())
        } else {
            format!("{}/{}", parent_path, name.to_str().unwrap())
        };

        let name_str = name.to_str().unwrap_or("");
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

        let client = BlockingClient::new();
        let token = self.token.clone();
        let base_url = self.base_url.trim_end_matches('/').to_string();
        let url = format!("{}/lookup/{}", base_url, path);

        match client.get(&url).bearer_auth(token).send() {
            Ok(r) if r.status().is_success() => match r.json::<FileInfo>() {
                Ok(obj) => {
                    println!("json {:?}", obj);
                    let kind = if obj.is_directory { FileType::Directory } else { FileType::RegularFile };
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
                    reply.entry(&Duration::new(60, 0), &attr, 0);
                }
                Err(_) => reply.error(ENOENT),
            },
            Ok(r) => {
                println!("lookup {} returned HTTP {}", url, r.status());
                reply.error(ENOENT)
            }
            Err(e) => {
                println!("HTTP lookup error {}: {}", url, e);
                reply.error(ENOENT)
            }
        }
    }

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

        let parent_path = self.get_path(parent).unwrap();
        let dir_name = name.to_str().unwrap_or("");
        let full_path = if parent_path == "/" { format!("/{}", dir_name) } else { format!("{}/{}", parent_path, dir_name) };

        let client = BlockingClient::new();
        let token = self.token.clone();
        let base_url = self.base_url.trim_end_matches('/').to_string();
        let url = format!("{}/mkdir/{}", base_url, full_path);

        match client.post(&url).bearer_auth(token).send() {
            Ok(r) if r.status().is_success() => {
                let ino = self.register_path(&full_path);
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
                // Non crea davvero nulla, ma fa contento il kernel
                reply.entry(&Duration::new(1, 0), &attr, 0);
            }
            _ => {
                // server call failed
                reply.error(EIO);
                return;
            }
        }
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
        let path= self.get_path(ino).unwrap();
        println!("setattr(ino={}, size={:?}, path={})", ino, size, path);
        
        //atributi dummy
        let ts = SystemTime::now();
        let mut attr = FileAttr {
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

    //   // Se viene richiesta una truncation, gestiscila (es. manda una chiamata al server)
    //     if let Some(_new_size) = size {
    //         println!(" caso truncation");
    //         //API CALL
    //         let client = Client::new();
    //         let token = self.token.clone();
    //         let base_url = self.base_url.clone();
    //         let res: Option<FileInfo>= task::block_in_place(|| {
    //         let rt = tokio::runtime::Handle::current();
    //             rt.block_on(async {
    //                 let resp = client
    //                     .get(format!("{}/lookup/{}", base_url, path)) 
    //                     .bearer_auth(token)
    //                     .send()
    //                     .await;

    //                 match resp {
    //                     Ok(r) if r.status().is_success() => r.json::<FileInfo>().await.ok(),
    //                     _ => None,
    //                 }
    //             })
    //         });

    //     match res {
    //         Some(obj) => {
    //             println!("json {:?}", obj);

    //             let kind= if obj.is_directory {
    //                 FileType::Directory
    //             } else {
    //                 FileType::RegularFile
    //             };

    //             let ts = parse_time(&obj.modified);
    //             attr = FileAttr {
    //             ino,
    //             size: obj.size,
    //             blocks: (obj.size / 512).max(1),
    //             atime: ts,
    //             mtime: ts,
    //             ctime: ts,
    //             crtime: ts,
    //             kind,
    //             perm: obj.permissions,
    //             nlink: obj.links,
    //             uid: self.uid,
    //             gid: self.gid,
    //             rdev: 0,
    //             flags: 0,
    //             blksize: 512,
    //         };
    //      }
    //     None => {//lookup fallita, il file ancora non esiste, vuole dire che è una write
    //         }
    //     }
       
    // }
        reply.attr(&Duration::from_secs(30), &attr);
        
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
        
        let path = match self.get_path(ino) {
            Some(p) => p,
            None => { reply.error(ENOENT); return; }
        };
        println!("execute write {}", path);
    
        self.write_buffer.append(& mut data.to_vec());
        // let client = Client::new();
        // let token = self.token.clone();
        // let base_url = self.base_url.clone();
        // let body = data.to_vec();

        // let ok: bool = task::block_in_place(|| {
        //     let rt = tokio::runtime::Handle::current();
        //     rt.block_on(async {
        //         let resp = client
        //             .put(format!("{}/files/{}", base_url, path))
        //             .bearer_auth(token)
        //             .body(body)
        //             .send()
        //             .await;

        //         match resp {
        //             Ok(r) => r.status().is_success(),
        //             Err(_) => false,
        //         }
        //     })
        // });

    
        reply.written(data.len() as u32);

    }

    fn flush(&mut self, _req: &Request, ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        let path = match self.get_path(ino) {
            Some(p) => p,
            None => { reply.error(ENOENT); return; }
        };
        let client = BlockingClient::new();
        let token = self.token.clone();
        let base_url = self.base_url.trim_end_matches('/').to_string();
        let body = self.write_buffer.clone();
        self.write_buffer.clear();
        // non svuotare il buffer prima della richiesta
        println!(" flush (ino={}, path={}, len={})", ino, path, body.len());

        if body.is_empty() {
            reply.ok();
            return;
        }

        match client.put(format!("{}/files/{}", base_url, path)).bearer_auth(token).body(body).send() {
            Ok(r) if r.status().is_success() => {
                // opzionale: aggiorna read buffer / size qui
                reply.ok();
            }
            Ok(r) => {
                println!("PUT returned HTTP {}", r.status());
                reply.error(EIO);
            }
            Err(e) => {
                println!("HTTP PUT error: {}", e);
                reply.error(EIO);
            }
        }
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        println!("unlink(parent={}, name={:?})", parent, name);
        let Some(parent_path) = self.get_path(parent) else { reply.error(ENOENT); return; };
        let full_path = format!("{}/{}", parent_path, name.to_str().unwrap());
        println!("Deleting {}", full_path);

        let client = BlockingClient::new();
        let token = self.token.clone();
        let base_url = self.base_url.trim_end_matches('/').to_string();

        match client.delete(format!("{}/files/{}", base_url, full_path)).bearer_auth(token).send() {
            Ok(r) if r.status().is_success() => reply.ok(),
            _ => reply.error(EIO),
        }
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        println!("rmdir(parent={}, name={:?})", parent, name);
        let Some(parent_path) = self.get_path(parent) else { reply.error(ENOENT); return; };
        let full_path = format!("{}/{}", parent_path, name.to_str().unwrap());
        println!("Removing directory {}", full_path);

        let client = BlockingClient::new();
        let token = self.token.clone();
        let base_url = self.base_url.trim_end_matches('/').to_string();

        match client.delete(format!("{}/files{}", base_url, full_path)).bearer_auth(token).send() {
            Ok(r) if r.status().is_success() => reply.ok(),
            _ => reply.error(EIO),
        }
    }

    
    
}


}
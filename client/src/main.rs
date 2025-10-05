use fuser::{Filesystem, FileAttr, FileType, ReplyAttr, ReplyData, ReplyDirectory, Request};
use libc::ENOENT;
use std::ffi::OsStr;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;

mod remote;

struct RemoteFs {
    base_url: String,
    rt: Runtime, // Tokio runtime per usare async in un contesto sync (FUSE)
}

impl Filesystem for RemoteFs {
    fn readdir(&mut self, _req: &Request, _ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        if offset == 0 {
            let entries = self.rt.block_on(remote::list_dir(&self.base_url, "/"));
            match entries {
                Ok(list) => {
                    let mut offset = 1;
                    for entry in list {
                        reply.add(entry.ino, offset, entry.kind, entry.name);
                        offset += 1;
                    }
                    reply.ok();
                }
                Err(_) => reply.error(ENOENT),
            }
        } else {
            reply.ok();
        }
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, reply: ReplyData) {
        let path = format!("/{}", ino); // mapping esempio
        let data = self.rt.block_on(remote::read_file(&self.base_url, &path));
        match data {
            Ok(content) => {
                let slice = &content[offset as usize..(offset as usize + size as usize).min(content.len())];
                reply.data(slice);
            }
            Err(_) => reply.error(ENOENT),
        }
    }
}

fn main() {
    let mountpoint = std::env::args().nth(1).expect("missing mountpoint");
    let fs = RemoteFs {
        base_url: "http://127.0.0.1:8080".into(),
        rt: Runtime::new().unwrap(),
    };
    fuser::mount2(fs, mountpoint, &[]).unwrap();
}

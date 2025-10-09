pub mod filesystem_mod{

use std::sync::{Arc, Mutex, Weak};
use std::ops::Deref;
use std::path::PathBuf;
use std::path::Path;
use std::fs::{self, OpenOptions};
use walkdir::WalkDir;
use rusqlite::{params, Connection, Result as SqlResult};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};


pub enum FSItem {
    File(File),
    Directory(Directory),
    SymLink(SymLink),
}

impl FSItem {
    // These methods allow us to use an FSItem in a uniform way
    // regardless of its actual type.
    pub fn name(&self) -> &str {
        match self {
            FSItem::File(f) => &f.name,
            FSItem::Directory(d) => &d.name,
            FSItem::SymLink(s) => &s.name,
        }
    }

    pub fn parent(&self) -> FSNodeWeak {
        match self {
            FSItem::File(f) => f.parent.clone(),
            FSItem::Directory(d) => d.parent.clone(),
            FSItem::SymLink(l) => l.parent.clone(),
        }
    }

    pub fn get_children(&self) -> Option<&Vec<FSNode>> {
        match self {
            FSItem::Directory(d) => Some(&d.children),
            _ => None,
        }
    }

    // can be called only if you are sure that self is a directory
    pub fn add(&mut self, item: FSNode) {
        match self {
            FSItem::Directory(d) => {
                d.children.push(item);
            }
            _ => panic!("Cannot add item to non-directory"),
        }
    }

    pub fn remove(&mut self, name: &str) {
        match self {
            FSItem::Directory(d) => {
                d.children.retain(|child| child.lock().unwrap().name() != name);
            }
            _ => panic!("Cannot remove item from non-directory"),
        }
    }

    pub fn set_name(&mut self, name: &str) {
        match self {
            FSItem::File(f) => f.name = name.to_owned(),
            FSItem::Directory(d) => d.name = name.to_owned(),
            FSItem::SymLink(s) => s.name = name.to_owned(),
        }
    }

    // return the absolute path of the item (of the parent)
    pub fn abs_path(&self) -> String {
        let mut parts = vec![];
        let mut current = self.parent().upgrade();

        while let Some(node) = current {
            let name = node.lock().unwrap().name().to_string();
            parts.insert(0, name);
            current = node.lock().unwrap().parent().upgrade();
        }

        if parts.len() < 2 {
            return "/".to_string();
        } else {
            return parts.join("/");
        }
    }


}

type FSItemCell = Mutex<FSItem>;
type FSNode = Arc<FSItemCell>;
type FSNodeWeak = Weak<FSItemCell>;

pub struct Permission {
    user: [char; 3],
    group: [char; 3],
    others: [char; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub file_id: Option<i64>,
    pub path: String,
    pub user_id: i64,
    pub user_permissions: u32,     // 0-7 (rwx)
    pub group_permissions: u32,    // 0-7 (rwx)
    pub others_permissions: u32,   // 0-7 (rwx)
    pub size: i64,
    pub created_at: String,
    pub last_modified: String,
}

impl FileMetadata {
    pub fn new(path: &str, user_id: i64, permissions: u32, is_directory: bool) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        
        let user_perms = (permissions >> 6) & 0o7;
        let group_perms = (permissions >> 3) & 0o7;
        let others_perms = permissions & 0o7;
        
        Self {
            file_id: None,
            path: path.to_string(),
            user_id,
            user_permissions: user_perms,
            group_permissions: group_perms,
            others_permissions: others_perms,
            size: 0,
            created_at: now.clone(),
            last_modified: now,
        }
    }
    
    pub fn get_octal_permissions(&self) -> u32 {
        (self.user_permissions << 6) + (self.group_permissions << 3) + self.others_permissions
    }

    pub fn update_modified_time(&mut self) {
            self.last_modified = chrono::Utc::now().to_rfc3339();
        }
}

// struct used to represent the informations of a file (the ones you want to see when you write ls -l)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub permissions: String,        // es: "drwxr-xr-x", "-rw-r--r--"
    pub links: u32,                 // always 1
    pub owner: String,              // owner username
    pub group: String,              // group (always users)
    pub size: i64,                  // dimension in bytes
    pub modified: String,           // last modifiied date
    pub name: String,               // name of the file/directory
    pub is_directory: bool,         // flag to identify wether it is a directory or not
}

impl FileInfo {
    pub fn new(
        permissions: String,
        owner: String,
        size: i64,
        modified: String,
        name: String,
        is_directory: bool,
    ) -> Self {
        Self {
            permissions,
            links: 1,  // always 1
            owner,
            group: "users".to_string(),  // always the same group "users"
            size,
            modified,
            name,
            is_directory,
        }
    }
}

pub struct File {
    name: String,
    size: usize,
    parent: FSNodeWeak,
}

pub struct Directory {
    name: String,
    parent: FSNodeWeak,
    children: Vec<FSNode>,
}

pub struct SymLink {
    name: String,
    target: String,
    parent: FSNodeWeak,
}

pub struct FileSystem {
    real_path: String,  // the real path of the file system
    root: FSNode,
    current: FSNode,
    side_effects: bool,  // enable / disable side effects on the file system
    db_connection: Option<Arc<Mutex<Connection>>>,
}

impl FileSystem {
    pub fn new() -> Self {
        let root = Arc::new(Mutex::new(FSItem::Directory(Directory {
            name: "".to_string(),
            parent: Weak::new(),
            children: vec![],
        })));

        FileSystem {
            real_path: ".".to_string(),
            root: root.clone(),
            current: root,
            side_effects: false,
            db_connection: None,
        }
    }

    // method to set the connection to the database
    pub fn set_database(&mut self, connection: Arc<Mutex<Connection>>) {
        self.db_connection = Some(connection);
    }

    // function to format permissions in the unix style
    fn format_permissions(user_perms: u32, group_perms: u32, others_perms: u32, is_directory: bool) -> String {
        let mut result = String::new();
        
        // Primo carattere: tipo di file
        result.push(if is_directory { 'd' } else { '-' });
        
        // Permessi user (owner)
        result.push(if user_perms & 4 != 0 { 'r' } else { '-' });
        result.push(if user_perms & 2 != 0 { 'w' } else { '-' });
        result.push(if user_perms & 1 != 0 { 'x' } else { '-' });
        
        // Permessi group
        result.push(if group_perms & 4 != 0 { 'r' } else { '-' });
        result.push(if group_perms & 2 != 0 { 'w' } else { '-' });
        result.push(if group_perms & 1 != 0 { 'x' } else { '-' });
        
        // Permessi others
        result.push(if others_perms & 4 != 0 { 'r' } else { '-' });
        result.push(if others_perms & 2 != 0 { 'w' } else { '-' });
        result.push(if others_perms & 1 != 0 { 'x' } else { '-' });
        
        result
    }

    fn format_timestamp(timestamp: &str) -> String {
        // Parse timestamp RFC3339 e formatta come "Dec  7 14:30"
        if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(timestamp) {
            datetime.format("%b %e %H:%M").to_string()
        } else {
            "Jan  1 00:00".to_string()  // Fallback
        }
    }

    fn get_username_by_id(&self, user_id: i64) -> Result<String, String> {
        if let Some(ref db) = self.db_connection {
            let conn = db.lock().unwrap();
            let mut stmt = conn.prepare("SELECT Username FROM USER WHERE User_ID = ?1")
                .map_err(|e| e.to_string())?;
            
            let username = stmt.query_row(params![user_id], |row| {
                Ok(row.get::<_, String>(0)?)
            }).optional().map_err(|e| e.to_string())?;
            
            Ok(username.unwrap_or_else(|| format!("user{}", user_id)))
        } else {
            Err("Database connection not initialized".to_string())
        }
    }

    // check if a user has the write permissions in a dir
    fn check_dir_write_permission(&self, dir_path: &str, user_id: i64) -> Result<(), String> {
        // Normalizza il path
        let normalized_path = if dir_path == "/" {
            "".to_string()
        } else {
            dir_path.trim_start_matches('/').trim_end_matches('/').to_string()
        };

        println!("🔐 Checking write permission for user {} in directory '{}'", user_id, normalized_path);

        // Verifica che la directory esista nel filesystem virtuale
        if self.find(&normalized_path).is_none() {
            return Err(format!("Directory '{}' not found", dir_path));
        }

        // Controlla i permessi nel database
        if let Some(ref db) = self.db_connection {
            let conn = db.lock().unwrap();
            
            let mut stmt = conn.prepare(
                "SELECT user_id, user_permissions, group_permissions, others_permissions, type 
                 FROM METADATA WHERE path = ?1"
            ).map_err(|e| format!("Database error: {}", e))?;
            
            let result = stmt.query_row(params![normalized_path], |row| {
                let owner_id: i64 = row.get(0)?;
                let user_perms: u32 = row.get(1)?;
                let group_perms: u32 = row.get(2)?;
                let others_perms: u32 = row.get(3)?;
                let file_type: i32 = row.get(4)?;
                
                Ok((owner_id, user_perms, group_perms, others_perms, file_type))
            });

            match result {
                Ok((owner_id, user_perms, group_perms, others_perms, file_type)) => {
                    // Verifica che sia una directory
                    if file_type != 1 {
                        return Err(format!("'{}' is not a directory", dir_path));
                    }

                    // Controlla permessi di scrittura (bit 2 = write permission)
                    let can_write = if owner_id == user_id {
                        // L'utente è il proprietario
                        let owner_can_write = (user_perms & 2) != 0;  // Bit 2 = write (-w-)
                        println!("   Owner check: user_perms={}, can_write={}", user_perms, owner_can_write);
                        owner_can_write
                    } else {
                        // L'utente NON è il proprietario, usa permessi "others"
                        let others_can_write = (others_perms & 2) != 0;  // Bit 2 = write (--w)
                        println!("   Others check: others_perms={}, can_write={}", others_perms, others_can_write);
                        others_can_write
                    };

                    if can_write {
                        println!("✅ Write permission granted for user {} in '{}'", user_id, dir_path);
                        Ok(())
                    } else {
                        println!("❌ Write permission denied for user {} in '{}'", user_id, dir_path);
                        Err(format!("Permission denied: no write access to directory '{}'", dir_path))
                    }
                },
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // Directory esiste nel filesystem ma non nel database
                    // Assumiamo permessi di default per compatibilità
                    println!("⚠️  Directory '{}' not found in metadata, allowing access for compatibility", normalized_path);
                    Ok(())
                },
                Err(e) => {
                    Err(format!("Database error checking permissions: {}", e))
                }
            }
        } else {
            // Nessuna connessione database, permetti l'operazione
            println!("⚠️  No database connection, allowing mkdir for compatibility");
            Ok(())
        }
    }


    pub fn from_file_system(base_path: &str) -> Self {
        
        let mut fs = FileSystem::new();
        fs.set_real_path(base_path);
        
        let wdir = WalkDir::new(base_path);
        for entry in wdir.into_iter()
                            .filter(|e| e.is_ok())
                            .map(|e| e.unwrap()) {
            let entry_path = entry.path();
                if entry_path == Path::new(base_path) {
                    // salta il root
                    continue;
                }
            
            // full fs path
            let _entry_path = entry.path().to_str().unwrap();
            let entry_path = PathBuf::from(_entry_path);
            
            // remove base path, get relative path
            let rel_path = entry_path.strip_prefix(base_path).unwrap();
            
            // split path in head / tail
            let head = if let Some(parent) = rel_path.parent() {
                "/".to_string() +  parent.to_str().unwrap()
            } else {
                "/".to_string()  
            };
           
            let name = entry_path.file_name().unwrap().to_str().unwrap();
            
            if entry_path.is_dir() {
                fs.make_dir(&head, name).unwrap();
            } else if entry_path.is_file() {
                fs.make_file(&head, name).unwrap();
            }
        }

        fs
    }

    pub fn set_real_path(&mut self, path: &str) {
        self.real_path = path.to_string();
    }


    fn make_real_path(&self, node: FSNode) -> String {
        
        let lock= node.lock().unwrap();
        let mut abs_path=lock.abs_path();
        let name= lock.name();

        while abs_path.starts_with("/") {
            abs_path = abs_path[1..].to_string();
            
        } 
       
        
        let real_path = PathBuf::from(&self.real_path)
            .join(&abs_path)
            .join(name);

        return real_path.to_str().unwrap().to_string();
    }

    //restituisce 
    fn split_path(path: &str) -> Vec<&str> {
        path.split('/').filter(|&t| t != "").collect()
    }

    pub fn find(&self, path: &str) -> Option<FSNode> {
        self.find_full(path, None)
    }

    // find using either absolute or relative path
    pub fn find_full(&self, path: &str, base: Option<&str>) -> Option<FSNode> {
        let parts = FileSystem::split_path(path);

        let mut current = if path.starts_with('/') {
            self.root.clone()
        } else {
            if let Some(base) = base {
                // if we can't find the base, return None
                self.find(base)?
            } else {
                self.current.clone()
            }
        };

        for part in parts {
            let next_node = match current.lock().unwrap().deref() {
                FSItem::Directory(d) => {
                    if part == "." {
                        current.clone()
                    }else if part == ".." {
                        match d.parent.upgrade() {
                            Some(parent) => parent,
                            None => return None, // if it tries to go over the root it returns none
                        }
                    } else {

                        // DEBUG: print current directory contents
                        /* 
                        for x in d.children.iter(){
                            println!("{:?}", x.lock().unwrap().name())
                        }
                        */

                        let item = d
                            .children
                            .iter()
                            .find(|&child| child.lock().unwrap().name() == part);

                        if let Some(item) = item {
                            item.clone()
                        } else {
                            return None;
                        }
                    }
                },
                FSItem::SymLink(link) => {
                    let path = current.lock().unwrap().abs_path();
                    let target = self.follow_link(&path, &link);
                    if let Some(target) = target {
                        target
                    } else {
                        return None;
                    }
                }
                FSItem::File(_) => {
                    return None;
                }
            };
            current = next_node;
        }
        Some(current)
    }

    pub fn follow_link(&self, path: &str, link: &SymLink) -> Option<FSNode> {

        // path is the absolute path of the link and it necessary if the link is relative

        let node = self.find_full(&link.target, Some(path));
        if let Some(node) = node {
            match node.lock().unwrap().deref() {
                FSItem::Directory(_) => return Some(node.clone()),
                FSItem::File(_) => return Some(node.clone()),
                FSItem::SymLink(ref link) => {
                    let path = node.lock().unwrap().abs_path();
                    return self.follow_link(&path, link)
                },
            }
        } else {
            return None
        }
    }

    pub fn change_dir(&mut self, path: &str) -> Result<(), String> {
        let node = self.find(path);
        if let Some(n) = node {
            self.current = n;
            Ok(())
        } else {
            Err(format!("Directory {} not found", path))
        }
    }

    pub fn list_contents(&self) -> Option<Vec<String>>{
        if let Some(res) = self.current.lock().unwrap().get_children(){
            Some(res.iter().map(|child| child.lock().unwrap().name().to_string()).collect())
        }
        else{
            None
        }
    }


    pub fn list_contents_with_metadata(&self, dir_path: &str, requesting_user_id: i64) -> Result<Vec<FileInfo>, String> {
        //  todo: come faccio a implementare una risposta NOTFOUND nel caso non ci sia la cartella. o un UNAUTHORIZED nel caso non si abbia il permesso in read per la cartella?
        println!("❓come faccio a implementare una risposta NOTFOUND nel caso non ci sia la cartella. E un UNAUTHORIZED nel caso non si abbia il permesso in read per la cartella?");

        if let Some(ref db) = self.db_connection {
            let conn = db.lock().unwrap();
            
            // Normalizza il path della directory
            let normalized_dir=dir_path.trim_start_matches('/').trim_end_matches('/');
            
            // query
            let mut stmt = conn.prepare(
                "SELECT m.path, m.user_id, m.user_permissions, m.group_permissions, m.others_permissions, 
                        m.size, m.last_modified, u.Username, m.type
                FROM METADATA m 
                LEFT JOIN USER u ON m.user_id = u.User_ID 
                WHERE m.path LIKE ?1
                ORDER BY m.path"
            ).map_err(|e| e.to_string())?;
            
            // like patern
            let like_pattern = if normalized_dir == "/" {
                "%".to_string()  // Tutti i file nella root
            } else {
                format!("{}%", normalized_dir)  // File nelle sottodirectory
            };
            
            let file_iter = stmt.query_map(params![like_pattern], |row| {
                let path: String = row.get(0)?;
                let user_id: i64 = row.get(1)?;
                let user_perms: u32 = row.get(2)?;
                let group_perms: u32 = row.get(3)?;
                let others_perms: u32 = row.get(4)?;
                let size: i64 = row.get(5)?;
                let last_modified: String = row.get(6)?;
                let username: Option<String> = row.get(7)?;
                let file_type: i32 = row.get(8)?;
                
                Ok((path, user_id, user_perms, group_perms, others_perms, size, last_modified, username, file_type))
            }).map_err(|e| e.to_string())?;
            
            let mut file_infos = Vec::new();
            
            for file_result in file_iter {
                let (path, user_id, user_perms, group_perms, others_perms, size, last_modified, username, file_type) = 
                    file_result.map_err(|e| e.to_string())?;
                
                let can_read = if user_id == requesting_user_id {
                    // L'utente è il proprietario del file
                    let owner_can_read = (user_perms & 4) != 0;  // Bit di lettura (r--)
                    owner_can_read
                } else {
                    // L'utente NON è il proprietario, usa permessi "others"
                    let others_can_read = (others_perms & 4) != 0;  // Bit di lettura (r--)
                    others_can_read
                };
                
                if can_read {
                    // ✅ FIX: Filtra i file che sono direttamente nella directory target
                    let should_include = if normalized_dir.is_empty() {
                        // Nella root ('') o ('/'): includi file senza slash nel nome
                        !path.contains('/')
                    } else {
                        // In sottodirectory: path deve iniziare con "dir/" e non avere altri slash dopo
                        let dir_with_slash = format!("{}/", normalized_dir);
                        path.starts_with(&dir_with_slash) && 
                        path[dir_with_slash.len()..].chars().filter(|&c| c == '/').count() == 0
                    };
                    
                    if should_include {
                        let file_name = path.split('/').last().unwrap_or("").to_string();
                        let is_directory = file_type == 1;  // 1 = directory, 0 = file
                        
                        let permissions = Self::format_permissions(user_perms, group_perms, others_perms, is_directory);
                        let formatted_time = Self::format_timestamp(&last_modified);
                        let owner = username.unwrap_or_else(|| format!("user{}", user_id));
                        
                        let file_info = FileInfo::new(
                            permissions,
                            owner,
                            size,
                            formatted_time,
                            file_name.clone(),
                            is_directory,
                        );
                        
                        file_infos.push(file_info);
                    }
                }
            }
            
            Ok(file_infos)
        } else {
            Err("Database connection not initialized".to_string())
        }
    }

    pub fn make_dir(&mut self, path: &str, name: &str) -> Result<(), String>{
        // Find the parent directory
        let node = self.find(path).ok_or_else(|| format!("Directory {} not found", path))?;

        // Check that it is a directory and that no child with the same name already exists
        {
            let lock = node.lock().unwrap();
            match &*lock {
                FSItem::Directory(d) => {
                    if d.children.iter().any(|child| child.lock().unwrap().name() == name) {
                        return Err(format!("Directory or file {} already exists in {}", name, path));
                    }
                }
                _ => return Err(format!("Invalid request, {} is not a directory", path)),
            }
        }

        // After checking everything is okay we can perform the modifications on the filesystem
        if self.side_effects {
            let real_path = self.make_real_path(node.clone());
            let target = PathBuf::from(&real_path).join(name);
            fs::create_dir(&target).map_err(|e| e.to_string())?;
        }

        // Creeate the new directory
        let new_dir = FSItem::Directory(Directory {
            name: name.to_string(),
            parent: Arc::downgrade(&node),
            children: vec![],
        });
        let new_node = Arc::new(Mutex::new(new_dir));

        // Add the new node and child
        {
            let mut lock = node.lock().unwrap();
            if let FSItem::Directory(d) = &mut *lock {
                d.children.push(new_node);
            }
        }

        Ok(())
    }

    // this is the version of the make_dir function that also updates the metadat inside the databse (so the one called by main.rs)
    pub fn make_dir_metadata(&mut self, path: &str, name: &str, user_id: i64, permissions: &str) -> Result<(), String> {
        // Verifica che l'utente abbia permessi di scrittura nella directory parent
        if let Err(e) = self.check_dir_write_permission(path, user_id) {
            return Err(e);
        }
        
        // Permessi da stringa ottale a numero
        let permissions_octal = u32::from_str_radix(permissions, 8)
            .map_err(|_| format!("Invalid permissions format: {}", permissions))?;
        
        if let Err(e) = self.make_dir(path, name) {
            return Err(e);
        }

        // path completo della directory (path + name)
        let full_path = if path == "/" {
            format!("{}", name)  // Nella root
        } else {
            let normalized_path = path.trim_end_matches('/');
            format!("{}/{}", normalized_path, name) 
        };

        // Salva i metadati nel database
        if let Some(ref db) = self.db_connection {
            let conn = db.lock().unwrap();
            let now = chrono::Utc::now().to_rfc3339();
            
            // Decompongo i permessi ottali in user/group/others
            let user_perms = (permissions_octal >> 6) & 0o7;
            let group_perms = (permissions_octal >> 3) & 0o7;
            let others_perms = permissions_octal & 0o7;
            
            let result = conn.execute(
                "INSERT INTO METADATA (path, user_id, user_permissions, group_permissions, others_permissions, size, created_at, last_modified, type)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    full_path,
                    user_id,
                    user_perms,
                    group_perms,
                    others_perms,
                    0,  // Size 0 per le directory
                    now.clone(),
                    now,
                    1,  // 1 = directory, 0 = file
                ],
            );
            
            if let Err(e) = result {
                println!("Warning: Failed to save directory metadata: {}", e);
                return Err(format!("Error: {}", e));
                // Non blocco l'operazione se il salvataggio metadati fallisce
            } else {
                println!("✅ Directory metadata saved: path='{}', user_id={}, permissions={}", full_path, user_id, permissions);
            }
        }

        Ok(())

        
    }

    // make file method
    pub fn make_file(&mut self, path: &str, name: &str) -> Result<(), String> {
        if let Some(node) = self.find(path) {
            
            if self.side_effects {
                // create the file on the file system
                let real_path = self.make_real_path(node.clone());
                let target = PathBuf::from(&real_path)
                    .join(name);
                fs::File::create(&target).map_err(|e| e.to_string())?;
            }

            let new_file = FSItem::File(File {
                name: name.to_string(),
                size: 0,
                parent: Arc::downgrade(&node),
            });

            let new_node = Arc::new(Mutex::new(new_file));
            node.lock().unwrap().add(new_node.clone());
            Ok(())
        }
        else {
            return Err(format!("Directory {} not found", path));
        }
    }

    // added for testing
    pub fn make_link(&mut self, path: &str, name: &str, target: &str) -> Result<(), String> {
        
        if let Some(node) = self.find(path) {

            // handle symlinks on FS only on linux
            #[cfg(target_os = "linux")]
            if self.side_effects {
                // create the link on the file system
                let real_path = self.make_real_path(node.clone());
                let link_path = PathBuf::from(&real_path)
                    .join(name);
                std::os::unix::fs::symlink(target, &link_path).map_err(|e| e.to_string())?;
            }

            let new_link = FSItem::SymLink(SymLink {
                name: name.to_string(),
                target: target.to_string(),
                parent: Arc::downgrade(&node),
            });

            let new_node = Arc::new(Mutex::new(new_link));
            node.lock().unwrap().add(new_node.clone());
            Ok(())
        } else {
            return Err(format!("Directory {} not found", path));
        }
    }

    pub fn rename(&self, path: &str, new_name: &str) -> Result<(), String> {
        let node = self.find(path);
        if let Some(n) = node {

            if self.side_effects {
                let real_path = self.make_real_path(n.clone());
                // dest
                let mut parts = real_path.split("/").collect::<Vec<&str>>();
                parts.pop(); 
                parts.push(new_name);// remove the last part (the file name)
                let new_path = parts.join("/");
                fs::rename(&real_path, &new_path).map_err(|e| e.to_string())?;
            }

            n.lock().unwrap().set_name(new_name);
            Ok(())
        } else {
            Err(format!("Item {} not found", path))
        }
    }

    pub fn delete(&self, path: &str, user_id: i64) -> Result<(), String> {
        let node:  Option<FSNode>  = self.find(path);
        if let Some(n) = node {

            // per eliminare un file o cartella si devono avere i permessi in scrittura sulla parent directory
            let path_ = Path::new(&path);
            let parent_dir = path_.parent().and_then(|p| p.to_str()).unwrap_or("");

            if let Err(e) = self.check_dir_write_permission(parent_dir, user_id) {
                return Err(e);
            }
            
            if self.side_effects {
                let item=n.lock().unwrap();
                match  *item{
                    FSItem::File(_) => {
                        drop(item);
                        let real_path = self.make_real_path(n.clone());
                        fs::remove_file(&real_path).map_err(|e| e.to_string())?;
                    }
                    FSItem::Directory(_) => {
                        drop(item);
                        let real_path = self.make_real_path(n.clone());
                        fs::remove_dir_all(&real_path).map_err(|e| e.to_string())?;
                        
                    }
                    FSItem::SymLink(_) => {
                        drop(item);
                        let real_path = self.make_real_path(n.clone());
                        fs::remove_file(&real_path).map_err(|e| e.to_string())?;
                    }
                }
            
            }

            // Remove from the database
            if let Err(e) = self.remove_from_database(path) {
                println!("Warning: Failed to remove metadata from database: {}", e);
                // Non blocco l'operazione se la rimozione dal database fallisce, si segnala solo un warning
            }

            let lock  = n.lock().unwrap();
            let name= (lock.name()).to_string();
            let par= lock.parent();
            if let Some(parent) = par.upgrade(){
                
                drop(lock);
                parent.lock().unwrap().remove(&name);
            }
           
            Ok(())
        } else {
            Err(format!("Item {} not found", path))
        }
        
    }

    pub fn set_side_effects(&mut self, side_effects: bool) {
        self.side_effects = side_effects;
    }

    fn remove_from_database(&self, item_path: &str) -> Result<(), String> {
        if let Some(ref db) = self.db_connection {
            let conn = db.lock().unwrap();
            let normalized_path = item_path.trim_start_matches('/');
            
            println!("🗄️  Removing from database: '{}'", normalized_path);
            
            // Controlla se è una directory
            let mut stmt = conn.prepare(
                "SELECT type FROM METADATA WHERE path = ?1"
            ).map_err(|e| format!("Database error: {}", e))?;
            
            let file_type = stmt.query_row(params![normalized_path], |row| {
                Ok(row.get::<_, i32>(0)?)
            }).optional().map_err(|e| format!("Database error: {}", e))?;
            
            match file_type {
                Some(1) => {
                    // Directory - rimuovi tutto il contenuto ricorsivamente
                    println!("📁 Removing directory and all contents from database");
                    
                    let delete_result = conn.execute(
                        "DELETE FROM METADATA WHERE path = ?1 OR path LIKE ?2",
                        params![normalized_path, format!("{}/%", normalized_path)],
                    );
                    
                    match delete_result {
                        Ok(rows_affected) => {
                            println!("✅ Removed {} items from database", rows_affected);
                            Ok(())
                        },
                        Err(e) => Err(format!("Failed to remove directory from database: {}", e))
                    }
                },
                Some(0) => {
                    // File - rimuovi solo questo
                    println!("📄 Removing file from database");
                    
                    let delete_result = conn.execute(
                        "DELETE FROM METADATA WHERE path = ?1",
                        params![normalized_path],
                    );
                    
                    match delete_result {
                        Ok(rows_affected) => {
                            println!("✅ Removed {} file(s) from database", rows_affected);
                            Ok(())
                        },
                        Err(e) => Err(format!("Failed to remove file from database: {}", e))
                    }
                },
                None => {
                    println!("⚠️  Item '{}' not found in database", normalized_path);
                    Ok(())
                },
                Some(t) => {
                    Err(format!("Unknown file type {} in database", t))
                }
            }
        } else {
            println!("⚠️  No database connection, skipping database removal");
            Ok(())
        }
    }

    pub fn write_file(&mut self, path: &str, content: &str, user_id: i64, permissions: &str) -> Result<(), String> {
        // NParsing permessi da stringa ottale a numero
        let permissions_octal = u32::from_str_radix(permissions, 8)
            .map_err(|_| format!("Invalid permissions format: {}", permissions))?;
        
        // Calcolo dimensione del contenuto
        let content_size = content.len() as i64;


        let node = self.find(path);
        if let Some(n) = node {
            
            let lock = n.lock().unwrap();
            match &*lock {
                FSItem::File(_) => {
                    if self.side_effects {
                        drop(lock);
                        let real_path = self.make_real_path(n.clone());
                        fs::write(&real_path, content).map_err(|e| e.to_string())?;
                        /*let mut file = OpenOptions::new()
                                                    .create(true)
                                                    .append(true)
                                                    .open(&real_path)
                                                    .map_err(|e| e.to_string())?;
                        file.write_all(content.as_bytes()).map_err(|e| e.to_string())?;*/

                        println!("File already existing");

                        if let Some(ref db) = self.db_connection {
                            let conn = db.lock().unwrap();
                            let now = chrono::Utc::now().to_rfc3339();
                            
                            let result = conn.execute(
                                "UPDATE METADATA SET size = ?1, last_modified = ?2 WHERE path = ?3",
                                params![content_size, now, path],
                            );
                            
                            if let Err(e) = result {
                                println!("Warning: Failed to update file metadata: {}", e);
                                // Non blocco l'operazione se l'update metadati fallisce
                            }
                        }
                    }
                    Ok(())
                },
                _ => Err(format!("Invalid request, {} is not a file", path)),
            }
        } else {
            //file not found, create it
            let path_buf = PathBuf::from(path);
            let path_parent=path_buf.parent().unwrap().to_str().unwrap();
            let file_name= path_buf.file_name().unwrap().to_str().unwrap();

            // in order to create a file we need to have the write permission on the directory
            if let Err(e) = self.check_dir_write_permission(path_parent, user_id) {
                return Err(e);
            }
            
            let parent= self.find(path_parent);
            if let Some(p)=parent{
                let lock= p.lock().unwrap();
                match lock.deref(){
                    FSItem::Directory(_) => {
                        drop(lock);

                        self.make_file(path_parent, file_name)?;

                        if self.side_effects {
                            let real_path_parent= self.make_real_path(p.clone());
                            let real_path= PathBuf::from(&real_path_parent).join(file_name);
                            fs::write(real_path, content).map_err(|e| e.to_string())?;
                        }

                        if let Some(ref db) = self.db_connection {
                            let conn = db.lock().unwrap();
                            let now = chrono::Utc::now().to_rfc3339();
                            
                            // NOTE: Decompongo i permessi ottali in user/group/others
                            let user_perms = (permissions_octal >> 6) & 0o7;
                            let group_perms = (permissions_octal >> 3) & 0o7;
                            let others_perms = permissions_octal & 0o7;
                            
                            let result = conn.execute(
                                "INSERT INTO METADATA (path, user_id, user_permissions, group_permissions, others_permissions, size, created_at, last_modified, type)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                                params![
                                    path,
                                    user_id,
                                    user_perms,
                                    group_perms,
                                    others_perms,
                                    content_size,
                                    now.clone(),
                                    now,
                                    0,
                                ],
                            );
                            
                            if let Err(e) = result {
                                println!("Warning: Failed to save file metadata: {}", e);
                                // NOTE: Non blocco l'operazione se il salvataggio metadati fallisce
                            }
                        }
                    },
                    _ => return Err(format!("Invalid request, {} is not a directory", path_parent)),
                    //COSA DEVE FARE SE è UN SymLink?
                    
                }   
                
            }else{
                return Err(format!("Directory {} not found", path_parent));
            }
            Ok(())
                
        }
           
    }
    
    pub fn read_file (&self, path: &str) -> Result<String, String> {
        let node = self.find(path);
        if let Some(n) = node {
            let lock = n.lock().unwrap();
            match &*lock {
                FSItem::File(_) => {
                    if self.side_effects {
                        drop(lock);
                        let real_path = self.make_real_path(n.clone());
                        let content = fs::read_to_string(&real_path).map_err(|e| e.to_string())?;
                        Ok(content)
                    } else {
                        Ok(String::new()) // if side effects are disabled, return empty content
                    }
                },
                _ => Err(format!("Invalid request, {} is not a file", path)),
            }
        } else {
            Err(format!("File {} not found", path))
        }
    }

}


}

pub use crate::filesystem_mod::FileSystem;


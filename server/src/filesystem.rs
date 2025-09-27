pub mod filesystem_mod{

use std::sync::{Arc, Mutex, Weak};
use std::ops::Deref;
use std::path::PathBuf;

use std::fs::{self, OpenOptions};

use walkdir::WalkDir;


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
    side_effects: bool  // enable / disable side effects on the file system
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
        }
    }

    pub fn from_file_system(base_path: &str) -> Self {
        
        let mut fs = FileSystem::new();
        fs.set_real_path(base_path);
        
        let wdir = WalkDir::new(base_path);
        for entry in wdir.into_iter()
            .filter(|e| e.is_ok())
            .map(|e| e.unwrap())  {
            
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
        let name= (lock.name()).clone();

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
                    } else if part == ".." {
                        d.parent.upgrade().unwrap()
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

    pub fn make_dir(&mut self, path: &str, name: &str) -> Result<(), String> {
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

        // Add the new node ad child
        {
            let mut lock = node.lock().unwrap();
            if let FSItem::Directory(d) = &mut *lock {
                d.children.push(new_node);
            }
        }

        Ok(())
    }

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

    pub fn delete(&self, path: &str) -> Result<(), String> {
        let node:  Option<FSNode>  = self.find(path);
        if let Some(n) = node {
            
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

            let lock  = n.lock().unwrap();
            let name= (lock.name()).clone().to_string();
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

    pub fn write_file(&mut self, path: &str, content: &str) -> Result<(), String> {
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
            
            let parent= self.find(path_parent);
            if let Some(p)=parent{
                let lock= p.lock().unwrap();
                match lock.deref(){
                    FSItem::Directory(_) => {
                        drop(lock);

                        self.make_file(path_parent, file_name)?;

                        let real_path_parent= self.make_real_path(p.clone());
                        let real_path= PathBuf::from(&real_path_parent).join(file_name);

                        fs::write(real_path, content).map_err(|e| e.to_string())?;
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


// UNIT TESTS

#[cfg(test)]
mod tests {
    use super::*;

    fn create_file_system_with_structure() -> FileSystem {
        let mut fs = FileSystem::new();
        fs.make_dir("/", "home").unwrap();
        fs.change_dir("/home").unwrap();
        fs.make_dir(".", "user").unwrap();
        fs.change_dir("./user").unwrap();
        fs.make_file(".", "file.txt").unwrap();
        fs.make_file(".", "file1.txt").unwrap();
        fs.make_dir("..", "user1").unwrap();
        fs.change_dir("../user1").unwrap();
        fs.make_file(".", "file.txt").unwrap();
        fs.make_link("/home", "link_user", "/home/user").unwrap();
        fs
    }

    #[test]
    fn create_basic_file_system() {
        let fs = FileSystem::new();
        assert_eq!(fs.root.lock().unwrap().name(), "");
    }

    #[test]
    fn create_directory() {
        let mut fs = FileSystem::new();
        fs.make_dir("/", "home").unwrap();
        let root = fs.root.lock().unwrap();
        if let Some(children) = root.get_children() {
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].lock().unwrap().name(), "home");
        } else {
            panic!("Root should have children");
        }
    }

}


}

pub use crate::filesystem_mod::FileSystem;


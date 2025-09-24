use server::filesystem_mod::*;


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
        assert_eq!(fs.root.borrow().name(), "");
    }

    #[test]
    fn create_directory() {
        let mut fs = FileSystem::new();
        fs.make_dir("/", "home").unwrap();
        let root = fs.root.borrow();
        if let Some(children) = root.get_children() {
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].borrow().name(), "home");
        } else {
            panic!("Root should have children");
        }
    }


    #[test]
    fn test_file_system() {
        let fs = create_file_system_with_structure();
        assert!(fs.find("/home/user/file1.txt").is_some());
        assert!(fs.find("/home/demo/file.txt").is_none());
        assert!(fs.find("/home/user1/file.txt").is_some());
    }


    #[test]
    fn test_follow_link() {
        let mut fs = create_file_system_with_structure();
        let link = fs.find("/home/link_user/file.txt");
        assert!(link.is_some());

        fs.make_link("/home", "dead_link", "/home/dead").unwrap();
        let link = fs.find("/home/dead_link/filed.txt");
        assert!(link.is_none());
    }

    #[test]
    fn test_side_effects() {
        let mut fs =  FileSystem::new();
        fs.set_side_effects(true);
        fs.set_real_path("/tmp"); //fs real path
        fs.make_dir("/", "test_dir").unwrap();
        fs.make_dir("/test_dir", "dir1").unwrap();
        fs.make_file("/test_dir/dir1", "file1.txt").unwrap();
        fs.make_file("/test_dir/dir1", "file2.txt").unwrap();
        fs.rename("/test_dir/dir1/file2.txt", "file3.txt").unwrap();
        fs.make_link("/test_dir/dir1", "link3.txt","./file3.txt").unwrap();
        fs.make_link("/test_dir/dir1", "link1.txt","./file1.txt").unwrap();
        fs.delete("/test_dir/dir1").unwrap();
        
        // uncommento to delete all
        // fs.delete("/test_dir").unwrap();
        
        assert!(true); 
    }

    #[test]
    fn test_from_file_system() {

        // adjust to your system
        let fs = FileSystem::from_file_system("/etc/apt");
        assert!(fs.find("/sources.list").is_some());
    }

}





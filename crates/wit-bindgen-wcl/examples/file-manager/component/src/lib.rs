wit_bindgen::generate!({
    path: "wit/file-manager.wit",
    world: "file-manager",
    exports: {
        "example:file-manager/fs-operations": FsOperations
    }
});

use exports::example::file_manager::fs_operations::*;
use example::file_manager::fs_host::{self, Permissions, OperationType};

struct FsOperations;

// Simulated file system data
static mut FILES: Vec<FileInfo> = Vec::new();

impl Guest for FsOperations {
    fn get_file_info(path: String) -> FileResult {
        fs_host::log_access(&path, OperationType::ReadFile, true);
        
        // Check if file exists in our simulated fs
        unsafe {
            if let Some(file) = FILES.iter().find(|f| f.path == path) {
                return FileResult::Success(file.clone());
            }
        }
        
        FileResult::NotFound
    }
    
    fn set_permissions(path: String, perms: Permissions) -> Result<(), String> {
        // Check if we have permission to modify
        if !fs_host::check_permission(&path, Permissions::WRITE) {
            fs_host::log_access(&path, OperationType::WriteFile, false);
            return Err("Permission denied: WRITE required".to_string());
        }
        
        fs_host::log_access(&path, OperationType::WriteFile, true);
        
        // Update permissions
        unsafe {
            if let Some(file) = FILES.iter_mut().find(|f| f.path == path) {
                file.permissions = perms;
                return Ok(());
            }
        }
        
        Err("File not found".to_string())
    }
    
    fn list_files(required_perms: Permissions) -> Vec<FileInfo> {
        unsafe {
            FILES.iter()
                .filter(|file| file.permissions.contains(required_perms))
                .cloned()
                .collect()
        }
    }
}

// Initialize some example files
#[no_mangle]
pub extern "C" fn _initialize() {
    unsafe {
        FILES.push(FileInfo {
            path: "/home/user/document.txt".to_string(),
            size: 1024,
            permissions: Permissions::READ | Permissions::WRITE,
            exists: true,
        });
        
        FILES.push(FileInfo {
            path: "/home/user/script.sh".to_string(),
            size: 512,
            permissions: Permissions::READ | Permissions::WRITE | Permissions::EXECUTE,
            exists: true,
        });
        
        FILES.push(FileInfo {
            path: "/home/user/readonly.conf".to_string(),
            size: 256,
            permissions: Permissions::READ,
            exists: true,
        });
        
        FILES.push(FileInfo {
            path: "/usr/bin/app".to_string(),
            size: 4096,
            permissions: Permissions::READ | Permissions::EXECUTE,
            exists: true,
        });
    }
}

// File Manager Host - Demonstrates flags and permissions
mod bindings;

use anyhow::Result;
use bindings::*;
use waclay::*;

// Host implementation for permission checking
struct FsHost;

impl bindings::FsHostHost for FsHost {
    fn check_permission(&mut self, path: String, perms: Permissions) -> bool {
        // Simple permission logic: allow most operations except DELETE on system paths
        if path.starts_with("/usr") || path.starts_with("/sys") {
            if perms.contains(Permissions::DELETE) {
                println!("  🔒 Permission check: DENIED (system path, no DELETE)");
                return false;
            }
        }
        
        println!("  ✅ Permission check: ALLOWED");
        true
    }
    
    fn log_access(&mut self, path: String, op: OperationType, allowed: bool) {
        let status = if allowed { "✅ ALLOWED" } else { "❌ DENIED" };
        let op_str = match op {
            OperationType::ReadFile => "READ",
            OperationType::WriteFile => "WRITE",
            OperationType::DeleteFile => "DELETE",
            OperationType::ExecuteFile => "EXECUTE",
        };
        println!("  📝 Access log: {} - {} - {}", path, op_str, status);
    }
}

fn main() -> Result<()> {
    println!("📁 File Manager Component Host");
    println!("================================\n");
    
    // Create engine with wasmi runtime
    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, FsHost);
    
    // Load the component
    let component_bytes = include_bytes!("../../component/component.wasm");
    let component = Component::new(&engine, component_bytes)?;
    println!("✅ Loaded file-manager component\n");
    
    // Create linker and register host functions
    let mut linker = Linker::default();
        // Register host functions
    imports::register_fs_host_host(&mut linker, &mut store)?;
    
    // Instantiate the component
    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Instantiated file-manager component\n");
    
    // Get the exported functions
    let get_file_info = exports_fs_operations::get_get_file_info(&instance, &mut store)?;
    let set_permissions = exports_fs_operations::get_set_permissions(&instance, &mut store)?;
    let list_files = exports_fs_operations::get_list_files(&instance, &mut store)?;
    
    println!("📊 Testing File Operations:");
    println!("----------------------------\n");
    
    // Test 1: Get file info
    println!("🔍 Test 1: Get file info for /home/user/document.txt");
    match get_file_info.call(&mut store, "/home/user/document.txt".to_string())? {
        FileResult::Success(info) => {
            println!("  Path: {}", info.path);
            println!("  Size: {} bytes", info.size);
            println!("  Permissions: {:?}", info.permissions);
            println!("  Exists: {}\n", info.exists);
        }
        FileResult::NotFound => println!("  File not found\n"),
        FileResult::PermissionDenied(msg) => println!("  Permission denied: {}\n", msg),
        FileResult::IoError(msg) => println!("  I/O error: {}\n", msg),
    }
    
    // Test 2: List files with READ permission
    println!("📄 Test 2: List files with READ permission");
    let readable = list_files.call(&mut store, Permissions::READ)?;
    println!("  Found {} readable files:", readable.len());
    for file in &readable {
        println!("    - {} (perms: {:?})", file.path, file.permissions);
    }
    println!();
    
    // Test 3: List executable files
    println!("🚀 Test 3: List files with EXECUTE permission");
    let executable = list_files.call(&mut store, Permissions::EXECUTE)?;
    println!("  Found {} executable files:", executable.len());
    for file in &executable {
        println!("    - {} (perms: {:?})", file.path, file.permissions);
    }
    println!();
    
    // Test 4: List files with both READ and WRITE
    println!("✏️  Test 4: List files with READ + WRITE permissions");
    let read_write = list_files.call(&mut store, Permissions::READ | Permissions::WRITE)?;
    println!("  Found {} read-write files:", read_write.len());
    for file in &read_write {
        println!("    - {} (perms: {:?})", file.path, file.permissions);
    }
    println!();
    
    // Test 5: Modify permissions
    println!("🔧 Test 5: Modify permissions for /home/user/document.txt");
    let new_perms = Permissions::READ | Permissions::EXECUTE;
    println!("  Setting permissions to: {:?}", new_perms);
    match set_permissions.call(&mut store, ("/home/user/document.txt".to_string(), new_perms))? {
        Ok(_) => {
            println!("  ✅ Permissions updated successfully!");
            
            // Verify the change
            if let FileResult::Success(info) = get_file_info.call(&mut store, "/home/user/document.txt".to_string())? {
                println!("  New permissions: {:?}\n", info.permissions);
            }
        }
        Err(msg) => println!("  ❌ Failed: {}\n", msg),
    }
    
    // Test 6: Try to modify system file
    println!("🚫 Test 6: Try to modify permissions on system file");
    println!("  Attempting to set WRITE on /usr/bin/app");
    match set_permissions.call(&mut store, ("/usr/bin/app".to_string(), Permissions::READ | Permissions::WRITE))? {
        Ok(_) => println!("  ✅ Permissions updated (unexpected!)\n"),
        Err(msg) => println!("  ❌ Failed as expected: {}\n", msg),
    }
    
    // Test 7: Get info for non-existent file
    println!("❓ Test 7: Get info for non-existent file");
    match get_file_info.call(&mut store, "/nonexistent/file.txt".to_string())? {
        FileResult::Success(_) => println!("  Unexpected success\n"),
        FileResult::NotFound => println!("  ✅ Correctly returned NotFound\n"),
        FileResult::PermissionDenied(msg) => println!("  Permission denied: {}\n", msg),
        FileResult::IoError(msg) => println!("  I/O error: {}\n", msg),
    }
    
    println!("✅ File manager demo completed successfully!");
    println!("\n💡 Key Features Demonstrated:");
    println!("   • Flags with bitwise operations (READ | WRITE | EXECUTE)");
    println!("   • Permission checking via host imports");
    println!("   • Variant types for complex error handling");
    println!("   • Record types with multiple fields");
    println!("   • Access logging through host callbacks");
    println!("   • Runtime-agnostic bindings (using wasmi)");
    
    Ok(())
}

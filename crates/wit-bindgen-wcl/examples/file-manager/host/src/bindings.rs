// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use waclay::*;
use wasm_runtime_layer::{backend};
use bitflags;

// Note: If using flags types, add to your Cargo.toml:
// bitflags = "2.0"


// ========== Type Definitions ==========

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Permissions: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
        const DELETE = 1 << 3;
    }
}

impl ComponentType for Permissions {
    fn ty() -> ValueType {
        ValueType::Flags(FlagsType::new(None, [
            "read",
            "write",
            "execute",
            "delete",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Flags(flags_val) = value {
            let mut result = Permissions::empty();
            let ty = flags_val.ty();
            let count = ty.names().len();
            for i in 0..count {
                if flags_val.get_index(i) {
                    result |= Permissions::from_bits_truncate(1 << i);
                }
            }
            Ok(result)
        } else {
            bail!("Expected Flags value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let flags_type = FlagsType::new(None, [
            "read",
            "write",
            "execute",
            "delete",
        ]).unwrap();

        let mut flags_val = Flags::new(flags_type);
        for i in 0..4 {
            if self.bits() & (1 << i) != 0 {
                flags_val.set_index(i, true);
            }
        }
        Ok(Value::Flags(flags_val))
    }
}

impl UnaryComponentType for Permissions {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    ReadFile,
    WriteFile,
    DeleteFile,
    ExecuteFile,
}

impl ComponentType for OperationType {
    fn ty() -> ValueType {
        ValueType::Enum(EnumType::new(None, [
            "read-file",
            "write-file",
            "delete-file",
            "execute-file",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Enum(enum_val) = value {
            let discriminant = enum_val.discriminant();
            match discriminant {
                0 => Ok(OperationType::ReadFile),
                1 => Ok(OperationType::WriteFile),
                2 => Ok(OperationType::DeleteFile),
                3 => Ok(OperationType::ExecuteFile),
                _ => bail!("Invalid enum discriminant: {}", discriminant),
            }
        } else {
            bail!("Expected Enum value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let enum_type = EnumType::new(None, [
            "read-file",
            "write-file",
            "delete-file",
            "execute-file",
        ]).unwrap();

        let discriminant = match self {
            OperationType::ReadFile => 0,
            OperationType::WriteFile => 1,
            OperationType::DeleteFile => 2,
            OperationType::ExecuteFile => 3,
        };

        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))
    }
}

impl UnaryComponentType for OperationType {}



#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
    pub permissions: Permissions,
    pub exists: bool,
}

impl ComponentType for FileInfo {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("path", ValueType::String),
                    ("size", ValueType::U64),
                    ("permissions", Permissions::ty()),
                    ("exists", ValueType::Bool),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let path = record
                .field("path")
                .ok_or_else(|| anyhow!("Missing 'path' field"))?;
            let size = record
                .field("size")
                .ok_or_else(|| anyhow!("Missing 'size' field"))?;
            let permissions = record
                .field("permissions")
                .ok_or_else(|| anyhow!("Missing 'permissions' field"))?;
            let exists = record
                .field("exists")
                .ok_or_else(|| anyhow!("Missing 'exists' field"))?;

            let path = if let Value::String(s) = path { s.to_string() } else { bail!("Expected string") };
            let size = if let Value::U64(x) = size { x } else { bail!("Expected u64") };
            let permissions = Permissions::from_value(&permissions)?;
            let exists = if let Value::Bool(x) = exists { x } else { bail!("Expected bool") };

            Ok(FileInfo {
                path,
                size,
                permissions,
                exists,
            })
        } else {
            bail!("Expected Record value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let record = Record::new(
            RecordType::new(
                None,
                [
                    ("path", ValueType::String),
                    ("size", ValueType::U64),
                    ("permissions", Permissions::ty()),
                    ("exists", ValueType::Bool),
                ],
            ).unwrap(),
            [
                ("path", Value::String(self.path.into())),
                ("size", Value::U64(self.size)),
                ("permissions", self.permissions.into_value()?),
                ("exists", Value::Bool(self.exists)),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for FileInfo {}

#[derive(Debug, Clone)]
pub enum FileResult {
    Success(FileInfo),
    PermissionDenied(String),
    NotFound,
    IoError(String),
}

impl ComponentType for FileResult {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("success", Some(FileInfo::ty())),
                    VariantCase::new("permission-denied", Some(ValueType::String)),
                    VariantCase::new("not-found", None),
                    VariantCase::new("io-error", Some(ValueType::String)),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Variant(variant) = value {
            let discriminant = variant.discriminant();
            let variant_ty = variant.ty();
            let case = &variant_ty.cases()[discriminant];
            let case_name = case.name();
            let payload = variant.value();

            match case_name {
                "success" => {
                    if let Some(payload_value) = payload {
                        let converted = FileInfo::from_value(&payload_value)?;
                        Ok(FileResult::Success(converted))
                    } else {
                        bail!("Expected payload for success case")
                    }
                }
                "permission-denied" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(FileResult::PermissionDenied(converted))
                    } else {
                        bail!("Expected payload for permission-denied case")
                    }
                }
                "not-found" => Ok(FileResult::NotFound),
                "io-error" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(FileResult::IoError(converted))
                    } else {
                        bail!("Expected payload for io-error case")
                    }
                }
                _ => bail!("Unknown variant case: {}", case_name),
            }
        } else {
            bail!("Expected Variant value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let variant_type = VariantType::new(
            None,
            [
                VariantCase::new("success", Some(FileInfo::ty())),
                VariantCase::new("permission-denied", Some(ValueType::String)),
                VariantCase::new("not-found", None),
                VariantCase::new("io-error", Some(ValueType::String)),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            FileResult::Success(val) => (0, Some(val.into_value()?)),
            FileResult::PermissionDenied(val) => (1, Some(Value::String(val.into()))),
            FileResult::NotFound => (2, None),
            FileResult::IoError(val) => (3, Some(Value::String(val.into()))),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for FileResult {}



// ========== Host Imports ==========

/// Host trait for interface: example:file-manager/fs-host
pub trait FsHostHost {
    fn check_permission(&mut self, path: String, perms: Permissions) -> bool;
    fn log_access(&mut self, path: String, op: OperationType, allowed: bool) -> ();
}

pub mod imports {
    use super::*;

    pub fn register_fs_host_host<T: FsHostHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        store: &mut Store<T, E>,
    ) -> Result<()> {
        let host_interface = linker
            .define_instance("example:file-manager/fs-host".try_into().unwrap())
            .context("Failed to define host interface")?;

        host_interface
            .define_func(
                "check-permission",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::String, Permissions::ty(), ],
                        [ValueType::Bool],
                    ),
                    |mut ctx, params, results| {
                        let path = if let Value::String(s) = &params[0] { s.to_string() } else { bail!("Expected string") };
                        let perms = Permissions::from_value(&params[1])?;
                        let result = ctx.data_mut().check_permission(path, perms);
                        results[0] = Value::Bool(result);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define check-permission function")?;

        host_interface
            .define_func(
                "log-access",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::String, OperationType::ty(), ValueType::Bool, ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let path = if let Value::String(s) = &params[0] { s.to_string() } else { bail!("Expected string") };
                        let op = OperationType::from_value(&params[1])?;
                        let allowed = if let Value::Bool(x) = &params[2] { *x } else { bail!("Expected bool") };
                        ctx.data_mut().log_access(path, op, allowed);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define log-access function")?;

        Ok(())
    }

}

// ========== Guest Exports ==========

pub mod exports_fs_operations {
    use super::*;

    pub const INTERFACE_NAME: &str = "example:file-manager/fs-operations";

    pub fn get_get_file_info<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<String, FileResult>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("get-file-info")
            .ok_or_else(|| anyhow!("Function 'get-file-info' not found"))?
            .typed::<String, FileResult>()
    }

    pub fn get_set_permissions<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(String, Permissions), Result<(), String>>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("set-permissions")
            .ok_or_else(|| anyhow!("Function 'set-permissions' not found"))?
            .typed::<(String, Permissions), Result<(), String>>()
    }

    pub fn get_list_files<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<Permissions, Vec<FileInfo>>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("list-files")
            .ok_or_else(|| anyhow!("Function 'list-files' not found"))?
            .typed::<Permissions, Vec<FileInfo>>()
    }

}


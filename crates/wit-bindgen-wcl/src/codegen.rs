use anyhow::Result;
use heck::{ToSnakeCase, ToUpperCamelCase};
use std::fmt::Write as FmtWrite;
use wit_parser::{
    Enum, Flags, Function, Interface, InterfaceId, Record, Resolve, Results, Type, TypeDef,
    TypeDefKind, Variant,
};

pub fn generate_type_definition(
    resolve: &Resolve,
    typedef: &TypeDef,
    rust_name: &str,
    output: &mut String,
) -> Result<()> {
    match &typedef.kind {
        TypeDefKind::Record(record) => {
            generate_record_type(resolve, rust_name, record, output)?;
        }
        TypeDefKind::Variant(variant) => {
            generate_variant_type(resolve, rust_name, variant, output)?;
        }
        TypeDefKind::Enum(enum_) => {
            generate_enum_type(rust_name, enum_, output)?;
        }
        TypeDefKind::Type(ty) => {
            // Type alias
            let rust_ty = type_to_rust_type(resolve, ty);
            // Skip recursive type aliases (pub type X = X) which occur with WIT "use" statements
            if rust_ty != rust_name {
                writeln!(output, "pub type {} = {};", rust_name, rust_ty)?;
            }
        }
        TypeDefKind::List(_)
        | TypeDefKind::Option(_)
        | TypeDefKind::Result(_)
        | TypeDefKind::Tuple(_) => {
            // These are handled inline, not as named types
            // They don't need separate definitions
        }
        TypeDefKind::Flags(flags) => {
            generate_flags_type(rust_name, flags, output)?;
        }
        TypeDefKind::Resource => {
            writeln!(
                output,
                "// TODO: Resource type '{}' - needs manual implementation",
                rust_name
            )?;
        }
        _ => {
            writeln!(output, "// TODO: Unsupported type kind for {}", rust_name)?;
        }
    }
    Ok(())
}

fn generate_record_type(
    resolve: &Resolve,
    rust_name: &str,
    record: &Record,
    output: &mut String,
) -> Result<()> {
    // Generate struct
    writeln!(output, "#[derive(Debug, Clone)]")?;
    writeln!(output, "pub struct {} {{", rust_name)?;
    for field in &record.fields {
        let field_name = field.name.to_snake_case();
        let field_type = type_to_rust_type(resolve, &field.ty);
        writeln!(output, "    pub {}: {},", field_name, field_type)?;
    }
    writeln!(output, "}}")?;
    writeln!(output)?;

    // Generate ComponentType implementation
    generate_record_component_type(resolve, rust_name, record, output)?;

    Ok(())
}

fn generate_record_component_type(
    resolve: &Resolve,
    rust_name: &str,
    record: &Record,
    output: &mut String,
) -> Result<()> {
    writeln!(output, "impl ComponentType for {} {{", rust_name)?;

    // ty() method
    writeln!(output, "    fn ty() -> ValueType {{")?;
    writeln!(output, "        ValueType::Record(")?;
    writeln!(output, "            RecordType::new(")?;
    writeln!(output, "                None,")?;
    writeln!(output, "                [")?;
    for field in &record.fields {
        let field_name = &field.name;
        let value_type = type_to_value_type(resolve, &field.ty);
        writeln!(
            output,
            "                    (\"{}\", {}),",
            field_name, value_type
        )?;
    }
    writeln!(output, "                ],")?;
    writeln!(output, "            ).unwrap(),")?;
    writeln!(output, "        )")?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    // from_value() method
    writeln!(
        output,
        "    fn from_value(value: &Value) -> Result<Self> {{"
    )?;
    writeln!(output, "        if let Value::Record(record) = value {{")?;

    // Extract fields
    for field in &record.fields {
        let field_name = field.name.to_snake_case();
        let wit_name = &field.name;
        writeln!(output, "            let {} = record", field_name)?;
        writeln!(output, "                .field(\"{}\")", wit_name)?;
        writeln!(
            output,
            "                .ok_or_else(|| anyhow!(\"Missing '{}' field\"))?;",
            wit_name
        )?;
    }
    writeln!(output)?;

    // Convert fields
    for field in &record.fields {
        let field_name = field.name.to_snake_case();
        generate_field_conversion(resolve, &field_name, &field.ty, output)?;
    }
    writeln!(output)?;

    // Return struct
    writeln!(output, "            Ok({} {{", rust_name)?;
    for field in &record.fields {
        let field_name = field.name.to_snake_case();
        writeln!(output, "                {},", field_name)?;
    }
    writeln!(output, "            }})")?;
    writeln!(output, "        }} else {{")?;
    writeln!(output, "            bail!(\"Expected Record value\")")?;
    writeln!(output, "        }}")?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    // into_value() method
    writeln!(output, "    fn into_value(self) -> Result<Value> {{")?;
    writeln!(output, "        let record = Record::new(")?;
    writeln!(output, "            RecordType::new(")?;
    writeln!(output, "                None,")?;
    writeln!(output, "                [")?;
    for field in &record.fields {
        let field_name = &field.name;
        let value_type = type_to_value_type(resolve, &field.ty);
        writeln!(
            output,
            "                    (\"{}\", {}),",
            field_name, value_type
        )?;
    }
    writeln!(output, "                ],")?;
    writeln!(output, "            ).unwrap(),")?;
    writeln!(output, "            [")?;
    for field in &record.fields {
        let field_name_snake = field.name.to_snake_case();
        let wit_name = &field.name;
        writeln!(
            output,
            "                (\"{}\", {}),",
            wit_name,
            field_to_value(resolve, &format!("self.{}", field_name_snake), &field.ty)
        )?;
    }
    writeln!(output, "            ],")?;
    writeln!(output, "        )?;")?;
    writeln!(output, "        Ok(Value::Record(record))")?;
    writeln!(output, "    }}")?;
    writeln!(output, "}}")?;
    writeln!(output)?;
    writeln!(output, "impl UnaryComponentType for {} {{}}", rust_name)?;

    Ok(())
}

fn generate_variant_type(
    resolve: &Resolve,
    rust_name: &str,
    variant: &Variant,
    output: &mut String,
) -> Result<()> {
    // Generate enum
    writeln!(output, "#[derive(Debug, Clone)]")?;
    writeln!(output, "pub enum {} {{", rust_name)?;
    for case in &variant.cases {
        let case_name = case.name.to_upper_camel_case();
        if let Some(ty) = &case.ty {
            let rust_ty = type_to_rust_type(resolve, ty);
            writeln!(output, "    {}({}),", case_name, rust_ty)?;
        } else {
            writeln!(output, "    {},", case_name)?;
        }
    }
    writeln!(output, "}}")?;
    writeln!(output)?;

    // Generate ComponentType implementation
    generate_variant_component_type(resolve, rust_name, variant, output)?;

    Ok(())
}

fn generate_variant_component_type(
    resolve: &Resolve,
    rust_name: &str,
    variant: &Variant,
    output: &mut String,
) -> Result<()> {
    writeln!(output, "impl ComponentType for {} {{", rust_name)?;

    // ty() method
    writeln!(output, "    fn ty() -> ValueType {{")?;
    writeln!(output, "        ValueType::Variant(")?;
    writeln!(output, "            VariantType::new(")?;
    writeln!(output, "                None,")?;
    writeln!(output, "                [")?;
    for case in &variant.cases {
        let case_name = &case.name;
        if let Some(ty) = &case.ty {
            let value_type = type_to_value_type(resolve, ty);
            writeln!(
                output,
                "                    VariantCase::new(\"{}\", Some({})),",
                case_name, value_type
            )?;
        } else {
            writeln!(
                output,
                "                    VariantCase::new(\"{}\", None),",
                case_name
            )?;
        }
    }
    writeln!(output, "                ],")?;
    writeln!(output, "            ).unwrap(),")?;
    writeln!(output, "        )")?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    // from_value() method
    writeln!(
        output,
        "    fn from_value(value: &Value) -> Result<Self> {{"
    )?;
    writeln!(output, "        if let Value::Variant(variant) = value {{")?;
    writeln!(
        output,
        "            let discriminant = variant.discriminant();"
    )?;
    writeln!(output, "            let variant_ty = variant.ty();")?;
    writeln!(
        output,
        "            let case = &variant_ty.cases()[discriminant];"
    )?;
    writeln!(output, "            let case_name = case.name();")?;
    writeln!(output, "            let payload = variant.value();")?;
    writeln!(output)?;
    writeln!(output, "            match case_name {{")?;

    for case in &variant.cases {
        let wit_name = &case.name;
        let rust_case = case.name.to_upper_camel_case();

        if let Some(ty) = &case.ty {
            writeln!(output, "                \"{}\" => {{", wit_name)?;
            writeln!(
                output,
                "                    if let Some(payload_value) = payload {{"
            )?;
            let conversion = value_to_rust(resolve, "payload_value", ty);
            writeln!(
                output,
                "                        let converted = {};",
                conversion
            )?;
            writeln!(
                output,
                "                        Ok({}::{}(converted))",
                rust_name, rust_case
            )?;
            writeln!(output, "                    }} else {{")?;
            writeln!(
                output,
                "                        bail!(\"Expected payload for {} case\")",
                wit_name
            )?;
            writeln!(output, "                    }}")?;
            writeln!(output, "                }}")?;
        } else {
            writeln!(
                output,
                "                \"{}\" => Ok({}::{}),",
                wit_name, rust_name, rust_case
            )?;
        }
    }

    writeln!(
        output,
        "                _ => bail!(\"Unknown variant case: {{}}\", case_name),"
    )?;
    writeln!(output, "            }}")?;
    writeln!(output, "        }} else {{")?;
    writeln!(output, "            bail!(\"Expected Variant value\")")?;
    writeln!(output, "        }}")?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    // into_value() method
    writeln!(output, "    fn into_value(self) -> Result<Value> {{")?;
    writeln!(output, "        let variant_type = VariantType::new(")?;
    writeln!(output, "            None,")?;
    writeln!(output, "            [")?;
    for case in &variant.cases {
        let case_name = &case.name;
        if let Some(ty) = &case.ty {
            let value_type = type_to_value_type(resolve, ty);
            writeln!(
                output,
                "                VariantCase::new(\"{}\", Some({})),",
                case_name, value_type
            )?;
        } else {
            writeln!(
                output,
                "                VariantCase::new(\"{}\", None),",
                case_name
            )?;
        }
    }
    writeln!(output, "            ],")?;
    writeln!(output, "        ).unwrap();")?;
    writeln!(output)?;
    writeln!(
        output,
        "        let (discriminant, payload) = match self {{"
    )?;

    for (idx, case) in variant.cases.iter().enumerate() {
        let rust_case = case.name.to_upper_camel_case();
        if let Some(ty) = &case.ty {
            writeln!(
                output,
                "            {}::{}(val) => ({}, Some({})),",
                rust_name,
                rust_case,
                idx,
                field_to_value(resolve, "val", ty)
            )?;
        } else {
            writeln!(
                output,
                "            {}::{} => ({}, None),",
                rust_name, rust_case, idx
            )?;
        }
    }

    writeln!(output, "        }};")?;
    writeln!(output)?;
    writeln!(
        output,
        "        let variant = Variant::new(variant_type, discriminant, payload)?;"
    )?;
    writeln!(output, "        Ok(Value::Variant(variant))")?;
    writeln!(output, "    }}")?;
    writeln!(output, "}}")?;
    writeln!(output)?;
    writeln!(output, "impl UnaryComponentType for {} {{}}", rust_name)?;

    Ok(())
}

fn generate_enum_type(rust_name: &str, enum_: &Enum, output: &mut String) -> Result<()> {
    writeln!(output, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]")?;
    writeln!(output, "pub enum {} {{", rust_name)?;
    for case in &enum_.cases {
        let case_name = case.name.to_upper_camel_case();
        writeln!(output, "    {},", case_name)?;
    }
    writeln!(output, "}}")?;
    writeln!(output)?;

    // Generate ComponentType implementation for enum
    writeln!(output, "impl ComponentType for {} {{", rust_name)?;

    // ty() method
    writeln!(output, "    fn ty() -> ValueType {{")?;
    writeln!(output, "        ValueType::Enum(EnumType::new(None, [")?;
    for case in &enum_.cases {
        writeln!(output, "            \"{}\",", case.name)?;
    }
    writeln!(output, "        ]).unwrap())")?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    // from_value() method
    writeln!(
        output,
        "    fn from_value(value: &Value) -> Result<Self> {{"
    )?;
    writeln!(output, "        if let Value::Enum(enum_val) = value {{")?;
    writeln!(
        output,
        "            let discriminant = enum_val.discriminant();"
    )?;
    writeln!(output, "            match discriminant {{")?;
    for (idx, case) in enum_.cases.iter().enumerate() {
        let case_name = case.name.to_upper_camel_case();
        writeln!(
            output,
            "                {} => Ok({}::{}),",
            idx, rust_name, case_name
        )?;
    }
    writeln!(
        output,
        "                _ => bail!(\"Invalid enum discriminant: {{}}\", discriminant),"
    )?;
    writeln!(output, "            }}")?;
    writeln!(output, "        }} else {{")?;
    writeln!(output, "            bail!(\"Expected Enum value\")")?;
    writeln!(output, "        }}")?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    // into_value() method
    writeln!(output, "    fn into_value(self) -> Result<Value> {{")?;
    writeln!(output, "        let enum_type = EnumType::new(None, [")?;
    for case in &enum_.cases {
        writeln!(output, "            \"{}\",", case.name)?;
    }
    writeln!(output, "        ]).unwrap();")?;
    writeln!(output)?;
    writeln!(output, "        let discriminant = match self {{")?;
    for (idx, case) in enum_.cases.iter().enumerate() {
        let case_name = case.name.to_upper_camel_case();
        writeln!(
            output,
            "            {}::{} => {},",
            rust_name, case_name, idx
        )?;
    }
    writeln!(output, "        }};")?;
    writeln!(output)?;
    writeln!(
        output,
        "        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))"
    )?;
    writeln!(output, "    }}")?;
    writeln!(output, "}}")?;
    writeln!(output)?;
    writeln!(output, "impl UnaryComponentType for {} {{}}", rust_name)?;

    Ok(())
}

fn generate_flags_type(rust_name: &str, flags: &Flags, output: &mut String) -> Result<()> {
    writeln!(output, "bitflags::bitflags! {{")?;
    writeln!(output, "    #[derive(Debug, Clone, Copy, PartialEq, Eq)]")?;
    writeln!(output, "    pub struct {}: u32 {{", rust_name)?;

    for (idx, flag) in flags.flags.iter().enumerate() {
        let _flag_name = flag.name.to_upper_camel_case();
        let flag_name_screaming = flag.name.to_uppercase().replace('-', "_");
        writeln!(
            output,
            "        const {} = 1 << {};",
            flag_name_screaming, idx
        )?;
    }

    writeln!(output, "    }}")?;
    writeln!(output, "}}")?;
    writeln!(output)?;

    // Generate ComponentType implementation
    writeln!(output, "impl ComponentType for {} {{", rust_name)?;

    // ty() method
    writeln!(output, "    fn ty() -> ValueType {{")?;
    writeln!(output, "        ValueType::Flags(FlagsType::new(None, [")?;
    for flag in &flags.flags {
        writeln!(output, "            \"{}\",", flag.name)?;
    }
    writeln!(output, "        ]).unwrap())")?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    // from_value() method
    writeln!(
        output,
        "    fn from_value(value: &Value) -> Result<Self> {{"
    )?;
    writeln!(output, "        if let Value::Flags(flags_val) = value {{")?;
    writeln!(
        output,
        "            let mut result = {}::empty();",
        rust_name
    )?;
    writeln!(output, "            let ty = flags_val.ty();")?;
    writeln!(output, "            let count = ty.names().len();")?;
    writeln!(output, "            for i in 0..count {{")?;
    writeln!(output, "                if flags_val.get_index(i) {{")?;
    writeln!(
        output,
        "                    result |= {}::from_bits_truncate(1 << i);",
        rust_name
    )?;
    writeln!(output, "                }}")?;
    writeln!(output, "            }}")?;
    writeln!(output, "            Ok(result)")?;
    writeln!(output, "        }} else {{")?;
    writeln!(output, "            bail!(\"Expected Flags value\")")?;
    writeln!(output, "        }}")?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    // into_value() method
    writeln!(output, "    fn into_value(self) -> Result<Value> {{")?;
    writeln!(output, "        let flags_type = FlagsType::new(None, [")?;
    for flag in &flags.flags {
        writeln!(output, "            \"{}\",", flag.name)?;
    }
    writeln!(output, "        ]).unwrap();")?;
    writeln!(output)?;
    writeln!(
        output,
        "        let mut flags_val = Flags::new(flags_type);"
    )?;
    writeln!(output, "        for i in 0..{} {{", flags.flags.len())?;
    writeln!(output, "            if self.bits() & (1 << i) != 0 {{")?;
    writeln!(output, "                flags_val.set_index(i, true);")?;
    writeln!(output, "            }}")?;
    writeln!(output, "        }}")?;
    writeln!(output, "        Ok(Value::Flags(flags_val))")?;
    writeln!(output, "    }}")?;
    writeln!(output, "}}")?;
    writeln!(output)?;
    writeln!(output, "impl UnaryComponentType for {} {{}}", rust_name)?;

    Ok(())
}

pub fn generate_import_trait(
    resolve: &Resolve,
    name: &str,
    interface: &Interface,
    _interface_id: InterfaceId,
    output: &mut String,
) -> Result<()> {
    let interface_name = interface
        .name
        .as_ref()
        .map(|n| n.to_upper_camel_case())
        .unwrap_or_else(|| "UnnamedInterface".to_string());

    let trait_name = format!("{}Host", interface_name);

    writeln!(output, "/// Host trait for interface: {}", name)?;
    writeln!(output, "pub trait {} {{", trait_name)?;

    for (func_name, func) in &interface.functions {
        generate_trait_method(resolve, func_name, func, output)?;
    }

    writeln!(output, "}}")?;
    writeln!(output)?;

    Ok(())
}

pub fn generate_import_registration_function(
    resolve: &Resolve,
    interface_key: &str,
    interface: &Interface,
    _interface_id: InterfaceId,
    output: &mut String,
) -> Result<()> {
    let interface_name = interface
        .name
        .as_ref()
        .map(|n| n.to_upper_camel_case())
        .unwrap_or_else(|| "UnnamedInterface".to_string());
    let trait_name = format!("{}Host", interface_name);

    generate_import_registration(resolve, interface_key, &trait_name, interface, output)?;

    Ok(())
}

fn generate_trait_method(
    resolve: &Resolve,
    func_name: &str,
    func: &Function,
    output: &mut String,
) -> Result<()> {
    let method_name = func_name.to_snake_case();

    // Build parameter list
    let mut params = vec!["&mut self".to_string()];
    for (param_name, param_ty) in func.params.iter() {
        let rust_ty = type_to_rust_type(resolve, param_ty);
        params.push(format!("{}: {}", param_name.to_snake_case(), rust_ty));
    }

    // Build return type
    let return_ty = match &func.results {
        Results::Named(results) if results.len() == 1 => type_to_rust_type(resolve, &results[0].1),
        Results::Anon(ty) => type_to_rust_type(resolve, ty),
        Results::Named(results) if results.len() > 1 => {
            let types: Vec<_> = results
                .iter()
                .map(|(_, ty)| type_to_rust_type(resolve, ty))
                .collect();
            format!("({})", types.join(", "))
        }
        _ => "()".to_string(),
    };

    writeln!(
        output,
        "    fn {}({}) -> {};",
        method_name,
        params.join(", "),
        return_ty
    )?;

    Ok(())
}

fn generate_import_registration(
    resolve: &Resolve,
    interface_key: &str,
    _trait_name: &str,
    interface: &Interface,
    output: &mut String,
) -> Result<()> {
    let interface_snake = interface
        .name
        .as_ref()
        .unwrap_or(&"interface".to_string())
        .to_snake_case();
    let interface_camel = interface
        .name
        .as_ref()
        .map(|n| n.to_upper_camel_case())
        .unwrap_or_else(|| "Interface".to_string());

    // Check if interface has functions
    let has_functions = !interface.functions.is_empty();
    let store_prefix = if has_functions { "" } else { "_" };
    let interface_prefix = if has_functions { "" } else { "_" };

    writeln!(
        output,
        "    pub fn register_{}_host<T: {}Host + 'static, E: backend::WasmEngine>(",
        interface_snake, interface_camel
    )?;
    writeln!(output, "        linker: &mut Linker,")?;
    writeln!(output, "        {}store: &mut Store<T, E>,", store_prefix)?;
    writeln!(output, "    ) -> Result<()> {{")?;
    writeln!(
        output,
        "        let {}host_interface = linker",
        interface_prefix
    )?;
    writeln!(
        output,
        "            .define_instance(\"{}\".try_into().unwrap())",
        interface_key
    )?;
    writeln!(
        output,
        "            .context(\"Failed to define host interface\")?;"
    )?;
    writeln!(output)?;

    // Register each function
    for (func_name, func) in &interface.functions {
        generate_function_registration(resolve, func_name, func, output)?;
    }

    writeln!(output, "        Ok(())")?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    Ok(())
}

fn generate_function_registration(
    resolve: &Resolve,
    func_name: &str,
    func: &Function,
    output: &mut String,
) -> Result<()> {
    writeln!(output, "        host_interface")?;
    writeln!(output, "            .define_func(")?;
    writeln!(output, "                \"{}\",", func_name)?;
    writeln!(output, "                Func::new(")?;
    writeln!(output, "                    &mut *store,")?;

    // FuncType
    writeln!(output, "                    FuncType::new(")?;
    write!(output, "                        [")?;
    for (_i, (_name, ty)) in func.params.iter().enumerate() {
        write!(output, "{}, ", type_to_value_type(resolve, ty))?;
    }
    writeln!(output, "],")?;

    write!(output, "                        [")?;
    match &func.results {
        Results::Named(results) => {
            for (_name, ty) in results.iter() {
                write!(output, "{}, ", type_to_value_type(resolve, ty))?;
            }
        }
        Results::Anon(ty) => {
            write!(output, "{}", type_to_value_type(resolve, ty))?;
        }
    }
    writeln!(output, "],")?;
    writeln!(output, "                    ),")?;

    // Closure - prefix 'results' with underscore if unused
    let results_param = if matches!(&func.results, Results::Named(r) if !r.is_empty())
        || matches!(&func.results, Results::Anon(_))
    {
        "results"
    } else {
        "_results"
    };
    writeln!(
        output,
        "                    |mut ctx, params, {}| {{",
        results_param
    )?;

    // Extract parameters
    for (i, (param_name, param_ty)) in func.params.iter().enumerate() {
        let param_snake = param_name.to_snake_case();
        writeln!(
            output,
            "                        let {} = {};",
            param_snake,
            value_to_rust(resolve, &format!("params[{}]", i), param_ty)
        )?;
    }

    // Call trait method
    let method_name = func_name.to_snake_case();
    let param_names: Vec<_> = func
        .params
        .iter()
        .map(|(name, _)| name.to_snake_case())
        .collect();

    if matches!(&func.results, Results::Named(r) if !r.is_empty())
        || matches!(&func.results, Results::Anon(_))
    {
        writeln!(
            output,
            "                        let result = ctx.data_mut().{}({});",
            method_name,
            param_names.join(", ")
        )?;
        let result_ty = match &func.results {
            Results::Anon(ty) => ty,
            Results::Named(r) if !r.is_empty() => &r[0].1,
            _ => unreachable!("Checked by outer condition"),
        };
        writeln!(
            output,
            "                        results[0] = {};",
            field_to_value(resolve, "result", result_ty)
        )?;
    } else {
        writeln!(
            output,
            "                        ctx.data_mut().{}({});",
            method_name,
            param_names.join(", ")
        )?;
    }

    writeln!(output, "                        Ok(())")?;
    writeln!(output, "                    }},")?;
    writeln!(output, "                ),")?;
    writeln!(output, "            )")?;
    writeln!(
        output,
        "            .context(\"Failed to define {} function\")?;",
        func_name
    )?;
    writeln!(output)?;

    Ok(())
}

pub fn generate_export_interface(
    resolve: &Resolve,
    interface_key: &str,
    interface: &Interface,
    output: &mut String,
) -> Result<()> {
    let interface_name = interface
        .name
        .as_ref()
        .map(|n| n.to_snake_case())
        .unwrap_or_else(|| "interface".to_string());

    writeln!(output, "pub mod exports_{} {{", interface_name)?;
    writeln!(output, "    use super::*;")?;
    writeln!(output)?;
    writeln!(
        output,
        "    pub const INTERFACE_NAME: &str = \"{}\";",
        interface_key
    )?;
    writeln!(output)?;

    for (func_name, func) in &interface.functions {
        generate_export_helper(resolve, func_name, func, output)?;
    }

    writeln!(output, "}}")?;
    writeln!(output)?;

    Ok(())
}

fn generate_export_helper(
    resolve: &Resolve,
    func_name: &str,
    func: &Function,
    output: &mut String,
) -> Result<()> {
    let fn_name = format!("get_{}", func_name.to_snake_case());

    // Build param tuple
    let param_tuple = if func.params.is_empty() {
        "()".to_string()
    } else if func.params.len() == 1 {
        type_to_rust_type(resolve, &func.params[0].1)
    } else {
        let types: Vec<_> = func
            .params
            .iter()
            .map(|(_, ty)| type_to_rust_type(resolve, ty))
            .collect();
        format!("({})", types.join(", "))
    };

    // Build result tuple - handle multiple returns properly
    let result_tuple = match &func.results {
        Results::Named(results) if results.is_empty() => "()".to_string(),
        Results::Named(results) if results.len() == 1 => {
            format!("({})", type_to_rust_type(resolve, &results[0].1))
        }
        Results::Named(results) => {
            let types: Vec<_> = results
                .iter()
                .map(|(_, ty)| type_to_rust_type(resolve, ty))
                .collect();
            format!("({})", types.join(", "))
        }
        Results::Anon(ty) => type_to_rust_type(resolve, ty),
    };

    writeln!(output, "    pub fn {}<T, E: backend::WasmEngine>(", fn_name)?;
    writeln!(output, "        instance: &Instance,")?;
    writeln!(output, "        _store: &mut Store<T, E>,")?;
    writeln!(
        output,
        "    ) -> Result<TypedFunc<{}, {}>> {{",
        param_tuple, result_tuple
    )?;
    writeln!(output, "        let interface = instance")?;
    writeln!(output, "            .exports()")?;
    writeln!(
        output,
        "            .instance(&INTERFACE_NAME.try_into().unwrap())"
    )?;
    writeln!(
        output,
        "            .ok_or_else(|| anyhow!(\"Interface not found\"))?;"
    )?;
    writeln!(output)?;
    writeln!(output, "        interface")?;
    writeln!(output, "            .func(\"{}\")", func_name)?;
    writeln!(
        output,
        "            .ok_or_else(|| anyhow!(\"Function '{}' not found\"))?",
        func_name
    )?;
    writeln!(
        output,
        "            .typed::<{}, {}>()",
        param_tuple, result_tuple
    )?;
    writeln!(output, "    }}")?;
    writeln!(output)?;

    Ok(())
}

// Helper functions for type conversion

fn type_to_rust_type(resolve: &Resolve, ty: &Type) -> String {
    match ty {
        Type::Bool => "bool".to_string(),
        Type::U8 => "u8".to_string(),
        Type::U16 => "u16".to_string(),
        Type::U32 => "u32".to_string(),
        Type::U64 => "u64".to_string(),
        Type::S8 => "i8".to_string(),
        Type::S16 => "i16".to_string(),
        Type::S32 => "i32".to_string(),
        Type::S64 => "i64".to_string(),
        Type::F32 => "f32".to_string(),
        Type::F64 => "f64".to_string(),
        Type::Char => "char".to_string(),
        Type::String => "String".to_string(),
        Type::Id(id) => {
            let typedef = &resolve.types[*id];
            // Check what kind of type this is
            match &typedef.kind {
                TypeDefKind::List(inner) => {
                    format!("Vec<{}>", type_to_rust_type(resolve, inner))
                }
                TypeDefKind::Option(inner) => {
                    format!("Option<{}>", type_to_rust_type(resolve, inner))
                }
                TypeDefKind::Result(result) => {
                    let ok = result
                        .ok
                        .as_ref()
                        .map(|t| type_to_rust_type(resolve, t))
                        .unwrap_or_else(|| "()".to_string());
                    let err = result
                        .err
                        .as_ref()
                        .map(|t| type_to_rust_type(resolve, t))
                        .unwrap_or_else(|| "()".to_string());
                    format!("Result<{}, {}>", ok, err)
                }
                TypeDefKind::Tuple(tuple) => {
                    let types: Vec<_> = tuple
                        .types
                        .iter()
                        .map(|t| type_to_rust_type(resolve, t))
                        .collect();
                    format!("({})", types.join(", "))
                }
                _ => {
                    // Named type (record, variant, enum, etc.)
                    typedef
                        .name
                        .as_ref()
                        .map(|n| n.to_upper_camel_case())
                        .unwrap_or_else(|| format!("Type{:?}", id))
                }
            }
        }
    }
}

fn type_to_value_type(resolve: &Resolve, ty: &Type) -> String {
    match ty {
        Type::Bool => "ValueType::Bool".to_string(),
        Type::U8 => "ValueType::U8".to_string(),
        Type::U16 => "ValueType::U16".to_string(),
        Type::U32 => "ValueType::U32".to_string(),
        Type::U64 => "ValueType::U64".to_string(),
        Type::S8 => "ValueType::S8".to_string(),
        Type::S16 => "ValueType::S16".to_string(),
        Type::S32 => "ValueType::S32".to_string(),
        Type::S64 => "ValueType::S64".to_string(),
        Type::F32 => "ValueType::F32".to_string(),
        Type::F64 => "ValueType::F64".to_string(),
        Type::Char => "ValueType::Char".to_string(),
        Type::String => "ValueType::String".to_string(),
        Type::Id(id) => {
            let typedef = &resolve.types[*id];
            match &typedef.kind {
                TypeDefKind::List(inner) => {
                    format!(
                        "ValueType::List(ListType::new({}))",
                        type_to_value_type(resolve, inner)
                    )
                }
                TypeDefKind::Option(inner) => {
                    format!(
                        "ValueType::Option(OptionType::new({}))",
                        type_to_value_type(resolve, inner)
                    )
                }
                TypeDefKind::Result(result) => {
                    let ok = result
                        .ok
                        .as_ref()
                        .map(|t| format!("Some({})", type_to_value_type(resolve, t)))
                        .unwrap_or_else(|| "None".to_string());
                    let err = result
                        .err
                        .as_ref()
                        .map(|t| format!("Some({})", type_to_value_type(resolve, t)))
                        .unwrap_or_else(|| "None".to_string());
                    format!("ValueType::Result(ResultType::new({}, {}))", ok, err)
                }
                TypeDefKind::Tuple(tuple) => {
                    let types: Vec<_> = tuple
                        .types
                        .iter()
                        .map(|t| type_to_value_type(resolve, t))
                        .collect();
                    format!(
                        "ValueType::Tuple(TupleType::new(None, [{}]))",
                        types.join(", ")
                    )
                }
                _ => {
                    // For named types (record, variant, enum), call their ty() method
                    format!("{}::ty()", type_to_rust_type(resolve, ty))
                }
            }
        }
    }
}

fn generate_field_conversion(
    resolve: &Resolve,
    field_name: &str,
    ty: &Type,
    output: &mut String,
) -> Result<()> {
    let conversion = value_to_rust(resolve, field_name, ty);
    writeln!(output, "            let {} = {};", field_name, conversion)?;
    Ok(())
}

fn value_to_rust(resolve: &Resolve, value_expr: &str, ty: &Type) -> String {
    // Check if we're accessing a slice element (params[i]) vs owned value
    let is_slice_access = value_expr.contains("params[");

    // Special handling for Type::Id that might be result, option, list, or named types
    if let Type::Id(id) = ty {
        let typedef = &resolve.types[*id];
        match &typedef.kind {
            TypeDefKind::Result(result_ty) => {
                // Result types can use from_value directly since Result<T, E> implements ComponentType
                // when T and E implement ComponentType
                let ok_ty = match &result_ty.ok {
                    Some(ty) => format!("{}", type_to_rust_type(resolve, ty)),
                    None => "()".to_string(),
                };
                let err_ty = match &result_ty.err {
                    Some(ty) => format!("{}", type_to_rust_type(resolve, ty)),
                    None => "()".to_string(),
                };

                let value_ref = if is_slice_access {
                    format!("&{}", value_expr)
                } else {
                    format!("&{}", value_expr)
                };

                return format!(
                    "Result::<{}, {}>::from_value({})?",
                    ok_ty, err_ty, value_ref
                );
            }
            TypeDefKind::Option(_) | TypeDefKind::List(_) | TypeDefKind::Tuple(_) => {
                // These should be handled by ComponentType::from_value
                // Need turbofish for generic types
                let rust_ty = type_to_rust_type(resolve, ty);
                let value_ref = if is_slice_access {
                    format!("&{}", value_expr)
                } else {
                    format!("&{}", value_expr)
                };
                // For tuple types, we need angle brackets around the entire type
                // because qualified paths require it: <(T, U)>::from_value
                let ty_for_call = if rust_ty.starts_with('(') {
                    format!("<{}>", rust_ty)
                } else {
                    // Add turbofish :: before the generic parameters for other types
                    rust_ty.replace("<", "::<")
                };
                return format!("{}::from_value({})?", ty_for_call, value_ref);
            }
            _ => {
                // Named types (records, variants, enums, etc.)
                let rust_ty = type_to_rust_type(resolve, ty);
                let value_ref = if is_slice_access {
                    format!("&{}", value_expr)
                } else {
                    format!("&{}", value_expr)
                };
                return format!("{}::from_value({})?", rust_ty, value_ref);
            }
        }
    }

    if is_slice_access {
        // For slice access, we match on &Value
        match ty {
            Type::Bool => format!("if let Value::Bool(x) = &{} {{ *x }} else {{ bail!(\"Expected bool\") }}", value_expr),
            Type::U8 => format!("if let Value::U8(x) = &{} {{ *x }} else {{ bail!(\"Expected u8\") }}", value_expr),
            Type::U16 => format!("if let Value::U16(x) = &{} {{ *x }} else {{ bail!(\"Expected u16\") }}", value_expr),
            Type::U32 => format!("if let Value::U32(x) = &{} {{ *x }} else {{ bail!(\"Expected u32\") }}", value_expr),
            Type::U64 => format!("if let Value::U64(x) = &{} {{ *x }} else {{ bail!(\"Expected u64\") }}", value_expr),
            Type::S8 => format!("if let Value::S8(x) = &{} {{ *x }} else {{ bail!(\"Expected s8\") }}", value_expr),
            Type::S16 => format!("if let Value::S16(x) = &{} {{ *x }} else {{ bail!(\"Expected s16\") }}", value_expr),
            Type::S32 => format!("if let Value::S32(x) = &{} {{ *x }} else {{ bail!(\"Expected s32\") }}", value_expr),
            Type::S64 => format!("if let Value::S64(x) = &{} {{ *x }} else {{ bail!(\"Expected s64\") }}", value_expr),
            Type::F32 => format!("if let Value::F32(x) = &{} {{ *x }} else {{ bail!(\"Expected f32\") }}", value_expr),
            Type::F64 => format!("if let Value::F64(x) = &{} {{ *x }} else {{ bail!(\"Expected f64\") }}", value_expr),
            Type::Char => format!("if let Value::Char(x) = &{} {{ *x }} else {{ bail!(\"Expected char\") }}", value_expr),
            Type::String => format!("if let Value::String(s) = &{} {{ s.to_string() }} else {{ bail!(\"Expected string\") }}", value_expr),
            Type::Id(_) => unreachable!("Type::Id handled above"),
        }
    } else {
        // For owned values (from record.field()), match on Value
        match ty {
            Type::Bool => format!("if let Value::Bool(x) = {} {{ x }} else {{ bail!(\"Expected bool\") }}", value_expr),
            Type::U8 => format!("if let Value::U8(x) = {} {{ x }} else {{ bail!(\"Expected u8\") }}", value_expr),
            Type::U16 => format!("if let Value::U16(x) = {} {{ x }} else {{ bail!(\"Expected u16\") }}", value_expr),
            Type::U32 => format!("if let Value::U32(x) = {} {{ x }} else {{ bail!(\"Expected u32\") }}", value_expr),
            Type::U64 => format!("if let Value::U64(x) = {} {{ x }} else {{ bail!(\"Expected u64\") }}", value_expr),
            Type::S8 => format!("if let Value::S8(x) = {} {{ x }} else {{ bail!(\"Expected s8\") }}", value_expr),
            Type::S16 => format!("if let Value::S16(x) = {} {{ x }} else {{ bail!(\"Expected s16\") }}", value_expr),
            Type::S32 => format!("if let Value::S32(x) = {} {{ x }} else {{ bail!(\"Expected s32\") }}", value_expr),
            Type::S64 => format!("if let Value::S64(x) = {} {{ x }} else {{ bail!(\"Expected s64\") }}", value_expr),
            Type::F32 => format!("if let Value::F32(x) = {} {{ x }} else {{ bail!(\"Expected f32\") }}", value_expr),
            Type::F64 => format!("if let Value::F64(x) = {} {{ x }} else {{ bail!(\"Expected f64\") }}", value_expr),
            Type::Char => format!("if let Value::Char(x) = {} {{ x }} else {{ bail!(\"Expected char\") }}", value_expr),
            Type::String => format!("if let Value::String(s) = {} {{ s.to_string() }} else {{ bail!(\"Expected string\") }}", value_expr),
            Type::Id(_) => unreachable!("Type::Id handled above"),
        }
    }
}

fn field_to_value(_resolve: &Resolve, field_expr: &str, ty: &Type) -> String {
    match ty {
        Type::Bool => format!("Value::Bool({})", field_expr),
        Type::U8 => format!("Value::U8({})", field_expr),
        Type::U16 => format!("Value::U16({})", field_expr),
        Type::U32 => format!("Value::U32({})", field_expr),
        Type::U64 => format!("Value::U64({})", field_expr),
        Type::S8 => format!("Value::S8({})", field_expr),
        Type::S16 => format!("Value::S16({})", field_expr),
        Type::S32 => format!("Value::S32({})", field_expr),
        Type::S64 => format!("Value::S64({})", field_expr),
        Type::F32 => format!("Value::F32({})", field_expr),
        Type::F64 => format!("Value::F64({})", field_expr),
        Type::Char => format!("Value::Char({})", field_expr),
        Type::String => format!("Value::String({}.into())", field_expr),
        Type::Id(_) => {
            format!("{}.into_value()?", field_expr)
        }
    }
}

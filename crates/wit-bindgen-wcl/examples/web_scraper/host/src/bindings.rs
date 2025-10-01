// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use wasm_component_layer::*;
use wasm_runtime_layer::{backend};


// ========== Type Definitions ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl ComponentType for HttpMethod {
    fn ty() -> ValueType {
        ValueType::Enum(EnumType::new(None, [
            "get",
            "post",
            "put",
            "delete",
            "patch",
            "head",
            "options",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Enum(enum_val) = value {
            let discriminant = enum_val.discriminant();
            match discriminant {
                0 => Ok(HttpMethod::Get),
                1 => Ok(HttpMethod::Post),
                2 => Ok(HttpMethod::Put),
                3 => Ok(HttpMethod::Delete),
                4 => Ok(HttpMethod::Patch),
                5 => Ok(HttpMethod::Head),
                6 => Ok(HttpMethod::Options),
                _ => bail!("Invalid enum discriminant: {}", discriminant),
            }
        } else {
            bail!("Expected Enum value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let enum_type = EnumType::new(None, [
            "get",
            "post",
            "put",
            "delete",
            "patch",
            "head",
            "options",
        ]).unwrap();

        let discriminant = match self {
            HttpMethod::Get => 0,
            HttpMethod::Post => 1,
            HttpMethod::Put => 2,
            HttpMethod::Delete => 3,
            HttpMethod::Patch => 4,
            HttpMethod::Head => 5,
            HttpMethod::Options => 6,
        };

        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))
    }
}

impl UnaryComponentType for HttpMethod {}

#[derive(Debug, Clone)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

impl ComponentType for HttpHeader {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("name", ValueType::String),
                    ("value", ValueType::String),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let name = record
                .field("name")
                .ok_or_else(|| anyhow!("Missing 'name' field"))?;
            let value = record
                .field("value")
                .ok_or_else(|| anyhow!("Missing 'value' field"))?;

            let name = if let Value::String(s) = name { s.to_string() } else { bail!("Expected string") };
            let value = if let Value::String(s) = value { s.to_string() } else { bail!("Expected string") };

            Ok(HttpHeader {
                name,
                value,
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
                    ("name", ValueType::String),
                    ("value", ValueType::String),
                ],
            ).unwrap(),
            [
                ("name", Value::String(self.name.into())),
                ("value", Value::String(self.value.into())),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for HttpHeader {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSitePolicy {
    Strict,
    Lax,
    None,
}

impl ComponentType for SameSitePolicy {
    fn ty() -> ValueType {
        ValueType::Enum(EnumType::new(None, [
            "strict",
            "lax",
            "none",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Enum(enum_val) = value {
            let discriminant = enum_val.discriminant();
            match discriminant {
                0 => Ok(SameSitePolicy::Strict),
                1 => Ok(SameSitePolicy::Lax),
                2 => Ok(SameSitePolicy::None),
                _ => bail!("Invalid enum discriminant: {}", discriminant),
            }
        } else {
            bail!("Expected Enum value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let enum_type = EnumType::new(None, [
            "strict",
            "lax",
            "none",
        ]).unwrap();

        let discriminant = match self {
            SameSitePolicy::Strict => 0,
            SameSitePolicy::Lax => 1,
            SameSitePolicy::None => 2,
        };

        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))
    }
}

impl UnaryComponentType for SameSitePolicy {}





#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<u64>,
    pub max_age: Option<u32>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<SameSitePolicy>,
}

impl ComponentType for Cookie {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("name", ValueType::String),
                    ("value", ValueType::String),
                    ("domain", ValueType::Option(OptionType::new(ValueType::String))),
                    ("path", ValueType::Option(OptionType::new(ValueType::String))),
                    ("expires", ValueType::Option(OptionType::new(ValueType::U64))),
                    ("max-age", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("secure", ValueType::Bool),
                    ("http-only", ValueType::Bool),
                    ("same-site", ValueType::Option(OptionType::new(SameSitePolicy::ty()))),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let name = record
                .field("name")
                .ok_or_else(|| anyhow!("Missing 'name' field"))?;
            let value = record
                .field("value")
                .ok_or_else(|| anyhow!("Missing 'value' field"))?;
            let domain = record
                .field("domain")
                .ok_or_else(|| anyhow!("Missing 'domain' field"))?;
            let path = record
                .field("path")
                .ok_or_else(|| anyhow!("Missing 'path' field"))?;
            let expires = record
                .field("expires")
                .ok_or_else(|| anyhow!("Missing 'expires' field"))?;
            let max_age = record
                .field("max-age")
                .ok_or_else(|| anyhow!("Missing 'max-age' field"))?;
            let secure = record
                .field("secure")
                .ok_or_else(|| anyhow!("Missing 'secure' field"))?;
            let http_only = record
                .field("http-only")
                .ok_or_else(|| anyhow!("Missing 'http-only' field"))?;
            let same_site = record
                .field("same-site")
                .ok_or_else(|| anyhow!("Missing 'same-site' field"))?;

            let name = if let Value::String(s) = name { s.to_string() } else { bail!("Expected string") };
            let value = if let Value::String(s) = value { s.to_string() } else { bail!("Expected string") };
            let domain = Option::<String>::from_value(&domain)?;
            let path = Option::<String>::from_value(&path)?;
            let expires = Option::<u64>::from_value(&expires)?;
            let max_age = Option::<u32>::from_value(&max_age)?;
            let secure = if let Value::Bool(x) = secure { x } else { bail!("Expected bool") };
            let http_only = if let Value::Bool(x) = http_only { x } else { bail!("Expected bool") };
            let same_site = Option::<SameSitePolicy>::from_value(&same_site)?;

            Ok(Cookie {
                name,
                value,
                domain,
                path,
                expires,
                max_age,
                secure,
                http_only,
                same_site,
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
                    ("name", ValueType::String),
                    ("value", ValueType::String),
                    ("domain", ValueType::Option(OptionType::new(ValueType::String))),
                    ("path", ValueType::Option(OptionType::new(ValueType::String))),
                    ("expires", ValueType::Option(OptionType::new(ValueType::U64))),
                    ("max-age", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("secure", ValueType::Bool),
                    ("http-only", ValueType::Bool),
                    ("same-site", ValueType::Option(OptionType::new(SameSitePolicy::ty()))),
                ],
            ).unwrap(),
            [
                ("name", Value::String(self.name.into())),
                ("value", Value::String(self.value.into())),
                ("domain", self.domain.into_value()?),
                ("path", self.path.into_value()?),
                ("expires", self.expires.into_value()?),
                ("max-age", self.max_age.into_value()?),
                ("secure", Value::Bool(self.secure)),
                ("http-only", Value::Bool(self.http_only)),
                ("same-site", self.same_site.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for Cookie {}




#[derive(Debug, Clone)]
pub enum RequestBody {
    Text(String),
    Json(String),
    Form(Vec<(String, String)>),
    Binary(Vec<u8>),
    Empty,
}

impl ComponentType for RequestBody {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("text", Some(ValueType::String)),
                    VariantCase::new("json", Some(ValueType::String)),
                    VariantCase::new("form", Some(ValueType::List(ListType::new(ValueType::Tuple(TupleType::new(None, [ValueType::String, ValueType::String])))))),
                    VariantCase::new("binary", Some(ValueType::List(ListType::new(ValueType::U8)))),
                    VariantCase::new("empty", None),
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
                "text" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(RequestBody::Text(converted))
                    } else {
                        bail!("Expected payload for text case")
                    }
                }
                "json" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(RequestBody::Json(converted))
                    } else {
                        bail!("Expected payload for json case")
                    }
                }
                "form" => {
                    if let Some(payload_value) = payload {
                        let converted = Vec::<(String, String)>::from_value(&payload_value)?;
                        Ok(RequestBody::Form(converted))
                    } else {
                        bail!("Expected payload for form case")
                    }
                }
                "binary" => {
                    if let Some(payload_value) = payload {
                        let converted = Vec::<u8>::from_value(&payload_value)?;
                        Ok(RequestBody::Binary(converted))
                    } else {
                        bail!("Expected payload for binary case")
                    }
                }
                "empty" => Ok(RequestBody::Empty),
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
                VariantCase::new("text", Some(ValueType::String)),
                VariantCase::new("json", Some(ValueType::String)),
                VariantCase::new("form", Some(ValueType::List(ListType::new(ValueType::Tuple(TupleType::new(None, [ValueType::String, ValueType::String])))))),
                VariantCase::new("binary", Some(ValueType::List(ListType::new(ValueType::U8)))),
                VariantCase::new("empty", None),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            RequestBody::Text(val) => (0, Some(Value::String(val.into()))),
            RequestBody::Json(val) => (1, Some(Value::String(val.into()))),
            RequestBody::Form(val) => (2, Some(val.into_value()?)),
            RequestBody::Binary(val) => (3, Some(val.into_value()?)),
            RequestBody::Empty => (4, None),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for RequestBody {}




#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<HttpHeader>,
    pub cookies: Vec<Cookie>,
    pub body: Option<RequestBody>,
    pub timeout_ms: Option<u32>,
    pub follow_redirects: bool,
    pub max_redirects: Option<u32>,
}

impl ComponentType for HttpRequest {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("method", HttpMethod::ty()),
                    ("url", ValueType::String),
                    ("headers", ValueType::List(ListType::new(HttpHeader::ty()))),
                    ("cookies", ValueType::List(ListType::new(Cookie::ty()))),
                    ("body", ValueType::Option(OptionType::new(RequestBody::ty()))),
                    ("timeout-ms", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("follow-redirects", ValueType::Bool),
                    ("max-redirects", ValueType::Option(OptionType::new(ValueType::U32))),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let method = record
                .field("method")
                .ok_or_else(|| anyhow!("Missing 'method' field"))?;
            let url = record
                .field("url")
                .ok_or_else(|| anyhow!("Missing 'url' field"))?;
            let headers = record
                .field("headers")
                .ok_or_else(|| anyhow!("Missing 'headers' field"))?;
            let cookies = record
                .field("cookies")
                .ok_or_else(|| anyhow!("Missing 'cookies' field"))?;
            let body = record
                .field("body")
                .ok_or_else(|| anyhow!("Missing 'body' field"))?;
            let timeout_ms = record
                .field("timeout-ms")
                .ok_or_else(|| anyhow!("Missing 'timeout-ms' field"))?;
            let follow_redirects = record
                .field("follow-redirects")
                .ok_or_else(|| anyhow!("Missing 'follow-redirects' field"))?;
            let max_redirects = record
                .field("max-redirects")
                .ok_or_else(|| anyhow!("Missing 'max-redirects' field"))?;

            let method = HttpMethod::from_value(&method)?;
            let url = if let Value::String(s) = url { s.to_string() } else { bail!("Expected string") };
            let headers = Vec::<HttpHeader>::from_value(&headers)?;
            let cookies = Vec::<Cookie>::from_value(&cookies)?;
            let body = Option::<RequestBody>::from_value(&body)?;
            let timeout_ms = Option::<u32>::from_value(&timeout_ms)?;
            let follow_redirects = if let Value::Bool(x) = follow_redirects { x } else { bail!("Expected bool") };
            let max_redirects = Option::<u32>::from_value(&max_redirects)?;

            Ok(HttpRequest {
                method,
                url,
                headers,
                cookies,
                body,
                timeout_ms,
                follow_redirects,
                max_redirects,
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
                    ("method", HttpMethod::ty()),
                    ("url", ValueType::String),
                    ("headers", ValueType::List(ListType::new(HttpHeader::ty()))),
                    ("cookies", ValueType::List(ListType::new(Cookie::ty()))),
                    ("body", ValueType::Option(OptionType::new(RequestBody::ty()))),
                    ("timeout-ms", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("follow-redirects", ValueType::Bool),
                    ("max-redirects", ValueType::Option(OptionType::new(ValueType::U32))),
                ],
            ).unwrap(),
            [
                ("method", self.method.into_value()?),
                ("url", Value::String(self.url.into())),
                ("headers", self.headers.into_value()?),
                ("cookies", self.cookies.into_value()?),
                ("body", self.body.into_value()?),
                ("timeout-ms", self.timeout_ms.into_value()?),
                ("follow-redirects", Value::Bool(self.follow_redirects)),
                ("max-redirects", self.max_redirects.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for HttpRequest {}

#[derive(Debug, Clone)]
pub struct HttpStatus {
    pub code: u32,
    pub text: String,
}

impl ComponentType for HttpStatus {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("code", ValueType::U32),
                    ("text", ValueType::String),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let code = record
                .field("code")
                .ok_or_else(|| anyhow!("Missing 'code' field"))?;
            let text = record
                .field("text")
                .ok_or_else(|| anyhow!("Missing 'text' field"))?;

            let code = if let Value::U32(x) = code { x } else { bail!("Expected u32") };
            let text = if let Value::String(s) = text { s.to_string() } else { bail!("Expected string") };

            Ok(HttpStatus {
                code,
                text,
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
                    ("code", ValueType::U32),
                    ("text", ValueType::String),
                ],
            ).unwrap(),
            [
                ("code", Value::U32(self.code)),
                ("text", Value::String(self.text.into())),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for HttpStatus {}

#[derive(Debug, Clone)]
pub enum ResponseData {
    Text(String),
    Binary(Vec<u8>),
}

impl ComponentType for ResponseData {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("text", Some(ValueType::String)),
                    VariantCase::new("binary", Some(ValueType::List(ListType::new(ValueType::U8)))),
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
                "text" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(ResponseData::Text(converted))
                    } else {
                        bail!("Expected payload for text case")
                    }
                }
                "binary" => {
                    if let Some(payload_value) = payload {
                        let converted = Vec::<u8>::from_value(&payload_value)?;
                        Ok(ResponseData::Binary(converted))
                    } else {
                        bail!("Expected payload for binary case")
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
                VariantCase::new("text", Some(ValueType::String)),
                VariantCase::new("binary", Some(ValueType::List(ListType::new(ValueType::U8)))),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            ResponseData::Text(val) => (0, Some(Value::String(val.into()))),
            ResponseData::Binary(val) => (1, Some(val.into_value()?)),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for ResponseData {}

#[derive(Debug, Clone)]
pub struct ResponseContent {
    pub content_type: String,
    pub data: ResponseData,
    pub encoding: Option<String>,
}

impl ComponentType for ResponseContent {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("content-type", ValueType::String),
                    ("data", ResponseData::ty()),
                    ("encoding", ValueType::Option(OptionType::new(ValueType::String))),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let content_type = record
                .field("content-type")
                .ok_or_else(|| anyhow!("Missing 'content-type' field"))?;
            let data = record
                .field("data")
                .ok_or_else(|| anyhow!("Missing 'data' field"))?;
            let encoding = record
                .field("encoding")
                .ok_or_else(|| anyhow!("Missing 'encoding' field"))?;

            let content_type = if let Value::String(s) = content_type { s.to_string() } else { bail!("Expected string") };
            let data = ResponseData::from_value(&data)?;
            let encoding = Option::<String>::from_value(&encoding)?;

            Ok(ResponseContent {
                content_type,
                data,
                encoding,
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
                    ("content-type", ValueType::String),
                    ("data", ResponseData::ty()),
                    ("encoding", ValueType::Option(OptionType::new(ValueType::String))),
                ],
            ).unwrap(),
            [
                ("content-type", Value::String(self.content_type.into())),
                ("data", self.data.into_value()?),
                ("encoding", self.encoding.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ResponseContent {}

#[derive(Debug, Clone)]
pub struct ResponseTiming {
    pub dns_lookup_ms: u32,
    pub tcp_connect_ms: u32,
    pub tls_handshake_ms: Option<u32>,
    pub time_to_first_byte_ms: u32,
    pub total_time_ms: u32,
}

impl ComponentType for ResponseTiming {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("dns-lookup-ms", ValueType::U32),
                    ("tcp-connect-ms", ValueType::U32),
                    ("tls-handshake-ms", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("time-to-first-byte-ms", ValueType::U32),
                    ("total-time-ms", ValueType::U32),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let dns_lookup_ms = record
                .field("dns-lookup-ms")
                .ok_or_else(|| anyhow!("Missing 'dns-lookup-ms' field"))?;
            let tcp_connect_ms = record
                .field("tcp-connect-ms")
                .ok_or_else(|| anyhow!("Missing 'tcp-connect-ms' field"))?;
            let tls_handshake_ms = record
                .field("tls-handshake-ms")
                .ok_or_else(|| anyhow!("Missing 'tls-handshake-ms' field"))?;
            let time_to_first_byte_ms = record
                .field("time-to-first-byte-ms")
                .ok_or_else(|| anyhow!("Missing 'time-to-first-byte-ms' field"))?;
            let total_time_ms = record
                .field("total-time-ms")
                .ok_or_else(|| anyhow!("Missing 'total-time-ms' field"))?;

            let dns_lookup_ms = if let Value::U32(x) = dns_lookup_ms { x } else { bail!("Expected u32") };
            let tcp_connect_ms = if let Value::U32(x) = tcp_connect_ms { x } else { bail!("Expected u32") };
            let tls_handshake_ms = Option::<u32>::from_value(&tls_handshake_ms)?;
            let time_to_first_byte_ms = if let Value::U32(x) = time_to_first_byte_ms { x } else { bail!("Expected u32") };
            let total_time_ms = if let Value::U32(x) = total_time_ms { x } else { bail!("Expected u32") };

            Ok(ResponseTiming {
                dns_lookup_ms,
                tcp_connect_ms,
                tls_handshake_ms,
                time_to_first_byte_ms,
                total_time_ms,
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
                    ("dns-lookup-ms", ValueType::U32),
                    ("tcp-connect-ms", ValueType::U32),
                    ("tls-handshake-ms", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("time-to-first-byte-ms", ValueType::U32),
                    ("total-time-ms", ValueType::U32),
                ],
            ).unwrap(),
            [
                ("dns-lookup-ms", Value::U32(self.dns_lookup_ms)),
                ("tcp-connect-ms", Value::U32(self.tcp_connect_ms)),
                ("tls-handshake-ms", self.tls_handshake_ms.into_value()?),
                ("time-to-first-byte-ms", Value::U32(self.time_to_first_byte_ms)),
                ("total-time-ms", Value::U32(self.total_time_ms)),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ResponseTiming {}



#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: HttpStatus,
    pub headers: Vec<HttpHeader>,
    pub cookies: Vec<Cookie>,
    pub content: Option<ResponseContent>,
    pub redirect_chain: Vec<String>,
    pub timing: ResponseTiming,
}

impl ComponentType for HttpResponse {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("status", HttpStatus::ty()),
                    ("headers", ValueType::List(ListType::new(HttpHeader::ty()))),
                    ("cookies", ValueType::List(ListType::new(Cookie::ty()))),
                    ("content", ValueType::Option(OptionType::new(ResponseContent::ty()))),
                    ("redirect-chain", ValueType::List(ListType::new(ValueType::String))),
                    ("timing", ResponseTiming::ty()),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let status = record
                .field("status")
                .ok_or_else(|| anyhow!("Missing 'status' field"))?;
            let headers = record
                .field("headers")
                .ok_or_else(|| anyhow!("Missing 'headers' field"))?;
            let cookies = record
                .field("cookies")
                .ok_or_else(|| anyhow!("Missing 'cookies' field"))?;
            let content = record
                .field("content")
                .ok_or_else(|| anyhow!("Missing 'content' field"))?;
            let redirect_chain = record
                .field("redirect-chain")
                .ok_or_else(|| anyhow!("Missing 'redirect-chain' field"))?;
            let timing = record
                .field("timing")
                .ok_or_else(|| anyhow!("Missing 'timing' field"))?;

            let status = HttpStatus::from_value(&status)?;
            let headers = Vec::<HttpHeader>::from_value(&headers)?;
            let cookies = Vec::<Cookie>::from_value(&cookies)?;
            let content = Option::<ResponseContent>::from_value(&content)?;
            let redirect_chain = Vec::<String>::from_value(&redirect_chain)?;
            let timing = ResponseTiming::from_value(&timing)?;

            Ok(HttpResponse {
                status,
                headers,
                cookies,
                content,
                redirect_chain,
                timing,
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
                    ("status", HttpStatus::ty()),
                    ("headers", ValueType::List(ListType::new(HttpHeader::ty()))),
                    ("cookies", ValueType::List(ListType::new(Cookie::ty()))),
                    ("content", ValueType::Option(OptionType::new(ResponseContent::ty()))),
                    ("redirect-chain", ValueType::List(ListType::new(ValueType::String))),
                    ("timing", ResponseTiming::ty()),
                ],
            ).unwrap(),
            [
                ("status", self.status.into_value()?),
                ("headers", self.headers.into_value()?),
                ("cookies", self.cookies.into_value()?),
                ("content", self.content.into_value()?),
                ("redirect-chain", self.redirect_chain.into_value()?),
                ("timing", self.timing.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for HttpResponse {}



#[derive(Debug, Clone)]
pub enum Selector {
    Id(String),
    Class(String),
    Tag(String),
    Attribute((String, Option<String>)),
    Complex(String),
}

impl ComponentType for Selector {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("id", Some(ValueType::String)),
                    VariantCase::new("class", Some(ValueType::String)),
                    VariantCase::new("tag", Some(ValueType::String)),
                    VariantCase::new("attribute", Some(ValueType::Tuple(TupleType::new(None, [ValueType::String, ValueType::Option(OptionType::new(ValueType::String))])))),
                    VariantCase::new("complex", Some(ValueType::String)),
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
                "id" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(Selector::Id(converted))
                    } else {
                        bail!("Expected payload for id case")
                    }
                }
                "class" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(Selector::Class(converted))
                    } else {
                        bail!("Expected payload for class case")
                    }
                }
                "tag" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(Selector::Tag(converted))
                    } else {
                        bail!("Expected payload for tag case")
                    }
                }
                "attribute" => {
                    if let Some(payload_value) = payload {
                        let converted = <(String, Option<String>)>::from_value(&payload_value)?;
                        Ok(Selector::Attribute(converted))
                    } else {
                        bail!("Expected payload for attribute case")
                    }
                }
                "complex" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(Selector::Complex(converted))
                    } else {
                        bail!("Expected payload for complex case")
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
                VariantCase::new("id", Some(ValueType::String)),
                VariantCase::new("class", Some(ValueType::String)),
                VariantCase::new("tag", Some(ValueType::String)),
                VariantCase::new("attribute", Some(ValueType::Tuple(TupleType::new(None, [ValueType::String, ValueType::Option(OptionType::new(ValueType::String))])))),
                VariantCase::new("complex", Some(ValueType::String)),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            Selector::Id(val) => (0, Some(Value::String(val.into()))),
            Selector::Class(val) => (1, Some(Value::String(val.into()))),
            Selector::Tag(val) => (2, Some(Value::String(val.into()))),
            Selector::Attribute(val) => (3, Some(val.into_value()?)),
            Selector::Complex(val) => (4, Some(Value::String(val.into()))),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for Selector {}

#[derive(Debug, Clone)]
pub struct ElementAttribute {
    pub name: String,
    pub value: String,
}

impl ComponentType for ElementAttribute {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("name", ValueType::String),
                    ("value", ValueType::String),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let name = record
                .field("name")
                .ok_or_else(|| anyhow!("Missing 'name' field"))?;
            let value = record
                .field("value")
                .ok_or_else(|| anyhow!("Missing 'value' field"))?;

            let name = if let Value::String(s) = name { s.to_string() } else { bail!("Expected string") };
            let value = if let Value::String(s) = value { s.to_string() } else { bail!("Expected string") };

            Ok(ElementAttribute {
                name,
                value,
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
                    ("name", ValueType::String),
                    ("value", ValueType::String),
                ],
            ).unwrap(),
            [
                ("name", Value::String(self.name.into())),
                ("value", Value::String(self.value.into())),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ElementAttribute {}

#[derive(Debug, Clone)]
pub struct ElementStyle {
    pub property: String,
    pub value: String,
    pub important: bool,
}

impl ComponentType for ElementStyle {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("property", ValueType::String),
                    ("value", ValueType::String),
                    ("important", ValueType::Bool),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let property = record
                .field("property")
                .ok_or_else(|| anyhow!("Missing 'property' field"))?;
            let value = record
                .field("value")
                .ok_or_else(|| anyhow!("Missing 'value' field"))?;
            let important = record
                .field("important")
                .ok_or_else(|| anyhow!("Missing 'important' field"))?;

            let property = if let Value::String(s) = property { s.to_string() } else { bail!("Expected string") };
            let value = if let Value::String(s) = value { s.to_string() } else { bail!("Expected string") };
            let important = if let Value::Bool(x) = important { x } else { bail!("Expected bool") };

            Ok(ElementStyle {
                property,
                value,
                important,
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
                    ("property", ValueType::String),
                    ("value", ValueType::String),
                    ("important", ValueType::Bool),
                ],
            ).unwrap(),
            [
                ("property", Value::String(self.property.into())),
                ("value", Value::String(self.value.into())),
                ("important", Value::Bool(self.important)),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ElementStyle {}



#[derive(Debug, Clone)]
pub struct DomElement {
    pub tag_name: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<ElementAttribute>,
    pub styles: Vec<ElementStyle>,
    pub text_content: Option<String>,
    pub inner_html: Option<String>,
    pub parent_path: Vec<String>,
    pub has_children: bool,
    pub child_count: u32,
}

impl ComponentType for DomElement {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("tag-name", ValueType::String),
                    ("id", ValueType::Option(OptionType::new(ValueType::String))),
                    ("classes", ValueType::List(ListType::new(ValueType::String))),
                    ("attributes", ValueType::List(ListType::new(ElementAttribute::ty()))),
                    ("styles", ValueType::List(ListType::new(ElementStyle::ty()))),
                    ("text-content", ValueType::Option(OptionType::new(ValueType::String))),
                    ("inner-html", ValueType::Option(OptionType::new(ValueType::String))),
                    ("parent-path", ValueType::List(ListType::new(ValueType::String))),
                    ("has-children", ValueType::Bool),
                    ("child-count", ValueType::U32),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let tag_name = record
                .field("tag-name")
                .ok_or_else(|| anyhow!("Missing 'tag-name' field"))?;
            let id = record
                .field("id")
                .ok_or_else(|| anyhow!("Missing 'id' field"))?;
            let classes = record
                .field("classes")
                .ok_or_else(|| anyhow!("Missing 'classes' field"))?;
            let attributes = record
                .field("attributes")
                .ok_or_else(|| anyhow!("Missing 'attributes' field"))?;
            let styles = record
                .field("styles")
                .ok_or_else(|| anyhow!("Missing 'styles' field"))?;
            let text_content = record
                .field("text-content")
                .ok_or_else(|| anyhow!("Missing 'text-content' field"))?;
            let inner_html = record
                .field("inner-html")
                .ok_or_else(|| anyhow!("Missing 'inner-html' field"))?;
            let parent_path = record
                .field("parent-path")
                .ok_or_else(|| anyhow!("Missing 'parent-path' field"))?;
            let has_children = record
                .field("has-children")
                .ok_or_else(|| anyhow!("Missing 'has-children' field"))?;
            let child_count = record
                .field("child-count")
                .ok_or_else(|| anyhow!("Missing 'child-count' field"))?;

            let tag_name = if let Value::String(s) = tag_name { s.to_string() } else { bail!("Expected string") };
            let id = Option::<String>::from_value(&id)?;
            let classes = Vec::<String>::from_value(&classes)?;
            let attributes = Vec::<ElementAttribute>::from_value(&attributes)?;
            let styles = Vec::<ElementStyle>::from_value(&styles)?;
            let text_content = Option::<String>::from_value(&text_content)?;
            let inner_html = Option::<String>::from_value(&inner_html)?;
            let parent_path = Vec::<String>::from_value(&parent_path)?;
            let has_children = if let Value::Bool(x) = has_children { x } else { bail!("Expected bool") };
            let child_count = if let Value::U32(x) = child_count { x } else { bail!("Expected u32") };

            Ok(DomElement {
                tag_name,
                id,
                classes,
                attributes,
                styles,
                text_content,
                inner_html,
                parent_path,
                has_children,
                child_count,
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
                    ("tag-name", ValueType::String),
                    ("id", ValueType::Option(OptionType::new(ValueType::String))),
                    ("classes", ValueType::List(ListType::new(ValueType::String))),
                    ("attributes", ValueType::List(ListType::new(ElementAttribute::ty()))),
                    ("styles", ValueType::List(ListType::new(ElementStyle::ty()))),
                    ("text-content", ValueType::Option(OptionType::new(ValueType::String))),
                    ("inner-html", ValueType::Option(OptionType::new(ValueType::String))),
                    ("parent-path", ValueType::List(ListType::new(ValueType::String))),
                    ("has-children", ValueType::Bool),
                    ("child-count", ValueType::U32),
                ],
            ).unwrap(),
            [
                ("tag-name", Value::String(self.tag_name.into())),
                ("id", self.id.into_value()?),
                ("classes", self.classes.into_value()?),
                ("attributes", self.attributes.into_value()?),
                ("styles", self.styles.into_value()?),
                ("text-content", self.text_content.into_value()?),
                ("inner-html", self.inner_html.into_value()?),
                ("parent-path", self.parent_path.into_value()?),
                ("has-children", Value::Bool(self.has_children)),
                ("child-count", Value::U32(self.child_count)),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for DomElement {}





#[derive(Debug, Clone)]
pub struct ScrapeTarget {
    pub url: String,
    pub selectors: Vec<Selector>,
    pub required_fields: Vec<String>,
    pub follow_links: bool,
    pub max_depth: u32,
    pub delay_ms: Option<u32>,
}

impl ComponentType for ScrapeTarget {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("url", ValueType::String),
                    ("selectors", ValueType::List(ListType::new(Selector::ty()))),
                    ("required-fields", ValueType::List(ListType::new(ValueType::String))),
                    ("follow-links", ValueType::Bool),
                    ("max-depth", ValueType::U32),
                    ("delay-ms", ValueType::Option(OptionType::new(ValueType::U32))),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let url = record
                .field("url")
                .ok_or_else(|| anyhow!("Missing 'url' field"))?;
            let selectors = record
                .field("selectors")
                .ok_or_else(|| anyhow!("Missing 'selectors' field"))?;
            let required_fields = record
                .field("required-fields")
                .ok_or_else(|| anyhow!("Missing 'required-fields' field"))?;
            let follow_links = record
                .field("follow-links")
                .ok_or_else(|| anyhow!("Missing 'follow-links' field"))?;
            let max_depth = record
                .field("max-depth")
                .ok_or_else(|| anyhow!("Missing 'max-depth' field"))?;
            let delay_ms = record
                .field("delay-ms")
                .ok_or_else(|| anyhow!("Missing 'delay-ms' field"))?;

            let url = if let Value::String(s) = url { s.to_string() } else { bail!("Expected string") };
            let selectors = Vec::<Selector>::from_value(&selectors)?;
            let required_fields = Vec::<String>::from_value(&required_fields)?;
            let follow_links = if let Value::Bool(x) = follow_links { x } else { bail!("Expected bool") };
            let max_depth = if let Value::U32(x) = max_depth { x } else { bail!("Expected u32") };
            let delay_ms = Option::<u32>::from_value(&delay_ms)?;

            Ok(ScrapeTarget {
                url,
                selectors,
                required_fields,
                follow_links,
                max_depth,
                delay_ms,
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
                    ("url", ValueType::String),
                    ("selectors", ValueType::List(ListType::new(Selector::ty()))),
                    ("required-fields", ValueType::List(ListType::new(ValueType::String))),
                    ("follow-links", ValueType::Bool),
                    ("max-depth", ValueType::U32),
                    ("delay-ms", ValueType::Option(OptionType::new(ValueType::U32))),
                ],
            ).unwrap(),
            [
                ("url", Value::String(self.url.into())),
                ("selectors", self.selectors.into_value()?),
                ("required-fields", self.required_fields.into_value()?),
                ("follow-links", Value::Bool(self.follow_links)),
                ("max-depth", Value::U32(self.max_depth)),
                ("delay-ms", self.delay_ms.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ScrapeTarget {}

#[derive(Debug, Clone)]
pub struct ImageData {
    pub url: String,
    pub alt_text: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Option<String>,
}

impl ComponentType for ImageData {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("url", ValueType::String),
                    ("alt-text", ValueType::Option(OptionType::new(ValueType::String))),
                    ("width", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("height", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("format", ValueType::Option(OptionType::new(ValueType::String))),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let url = record
                .field("url")
                .ok_or_else(|| anyhow!("Missing 'url' field"))?;
            let alt_text = record
                .field("alt-text")
                .ok_or_else(|| anyhow!("Missing 'alt-text' field"))?;
            let width = record
                .field("width")
                .ok_or_else(|| anyhow!("Missing 'width' field"))?;
            let height = record
                .field("height")
                .ok_or_else(|| anyhow!("Missing 'height' field"))?;
            let format = record
                .field("format")
                .ok_or_else(|| anyhow!("Missing 'format' field"))?;

            let url = if let Value::String(s) = url { s.to_string() } else { bail!("Expected string") };
            let alt_text = Option::<String>::from_value(&alt_text)?;
            let width = Option::<u32>::from_value(&width)?;
            let height = Option::<u32>::from_value(&height)?;
            let format = Option::<String>::from_value(&format)?;

            Ok(ImageData {
                url,
                alt_text,
                width,
                height,
                format,
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
                    ("url", ValueType::String),
                    ("alt-text", ValueType::Option(OptionType::new(ValueType::String))),
                    ("width", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("height", ValueType::Option(OptionType::new(ValueType::U32))),
                    ("format", ValueType::Option(OptionType::new(ValueType::String))),
                ],
            ).unwrap(),
            [
                ("url", Value::String(self.url.into())),
                ("alt-text", self.alt_text.into_value()?),
                ("width", self.width.into_value()?),
                ("height", self.height.into_value()?),
                ("format", self.format.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ImageData {}

#[derive(Debug, Clone)]
pub enum ExtractedValue {
    Text(String),
    Number(f64),
    Boolean(bool),
    Url(String),
    Image(ImageData),
    Structured(Vec<(String, String)>),
}

impl ComponentType for ExtractedValue {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("text", Some(ValueType::String)),
                    VariantCase::new("number", Some(ValueType::F64)),
                    VariantCase::new("boolean", Some(ValueType::Bool)),
                    VariantCase::new("url", Some(ValueType::String)),
                    VariantCase::new("image", Some(ImageData::ty())),
                    VariantCase::new("structured", Some(ValueType::List(ListType::new(ValueType::Tuple(TupleType::new(None, [ValueType::String, ValueType::String])))))),
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
                "text" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(ExtractedValue::Text(converted))
                    } else {
                        bail!("Expected payload for text case")
                    }
                }
                "number" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::F64(x) = payload_value { x } else { bail!("Expected f64") };
                        Ok(ExtractedValue::Number(converted))
                    } else {
                        bail!("Expected payload for number case")
                    }
                }
                "boolean" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::Bool(x) = payload_value { x } else { bail!("Expected bool") };
                        Ok(ExtractedValue::Boolean(converted))
                    } else {
                        bail!("Expected payload for boolean case")
                    }
                }
                "url" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(ExtractedValue::Url(converted))
                    } else {
                        bail!("Expected payload for url case")
                    }
                }
                "image" => {
                    if let Some(payload_value) = payload {
                        let converted = ImageData::from_value(&payload_value)?;
                        Ok(ExtractedValue::Image(converted))
                    } else {
                        bail!("Expected payload for image case")
                    }
                }
                "structured" => {
                    if let Some(payload_value) = payload {
                        let converted = Vec::<(String, String)>::from_value(&payload_value)?;
                        Ok(ExtractedValue::Structured(converted))
                    } else {
                        bail!("Expected payload for structured case")
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
                VariantCase::new("text", Some(ValueType::String)),
                VariantCase::new("number", Some(ValueType::F64)),
                VariantCase::new("boolean", Some(ValueType::Bool)),
                VariantCase::new("url", Some(ValueType::String)),
                VariantCase::new("image", Some(ImageData::ty())),
                VariantCase::new("structured", Some(ValueType::List(ListType::new(ValueType::Tuple(TupleType::new(None, [ValueType::String, ValueType::String])))))),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            ExtractedValue::Text(val) => (0, Some(Value::String(val.into()))),
            ExtractedValue::Number(val) => (1, Some(Value::F64(val))),
            ExtractedValue::Boolean(val) => (2, Some(Value::Bool(val))),
            ExtractedValue::Url(val) => (3, Some(Value::String(val.into()))),
            ExtractedValue::Image(val) => (4, Some(val.into_value()?)),
            ExtractedValue::Structured(val) => (5, Some(val.into_value()?)),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for ExtractedValue {}

#[derive(Debug, Clone)]
pub struct ExtractedData {
    pub field_name: String,
    pub value: ExtractedValue,
    pub source_url: String,
    pub xpath: Option<String>,
    pub confidence: f32,
}

impl ComponentType for ExtractedData {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("field-name", ValueType::String),
                    ("value", ExtractedValue::ty()),
                    ("source-url", ValueType::String),
                    ("xpath", ValueType::Option(OptionType::new(ValueType::String))),
                    ("confidence", ValueType::F32),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let field_name = record
                .field("field-name")
                .ok_or_else(|| anyhow!("Missing 'field-name' field"))?;
            let value = record
                .field("value")
                .ok_or_else(|| anyhow!("Missing 'value' field"))?;
            let source_url = record
                .field("source-url")
                .ok_or_else(|| anyhow!("Missing 'source-url' field"))?;
            let xpath = record
                .field("xpath")
                .ok_or_else(|| anyhow!("Missing 'xpath' field"))?;
            let confidence = record
                .field("confidence")
                .ok_or_else(|| anyhow!("Missing 'confidence' field"))?;

            let field_name = if let Value::String(s) = field_name { s.to_string() } else { bail!("Expected string") };
            let value = ExtractedValue::from_value(&value)?;
            let source_url = if let Value::String(s) = source_url { s.to_string() } else { bail!("Expected string") };
            let xpath = Option::<String>::from_value(&xpath)?;
            let confidence = if let Value::F32(x) = confidence { x } else { bail!("Expected f32") };

            Ok(ExtractedData {
                field_name,
                value,
                source_url,
                xpath,
                confidence,
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
                    ("field-name", ValueType::String),
                    ("value", ExtractedValue::ty()),
                    ("source-url", ValueType::String),
                    ("xpath", ValueType::Option(OptionType::new(ValueType::String))),
                    ("confidence", ValueType::F32),
                ],
            ).unwrap(),
            [
                ("field-name", Value::String(self.field_name.into())),
                ("value", self.value.into_value()?),
                ("source-url", Value::String(self.source_url.into())),
                ("xpath", self.xpath.into_value()?),
                ("confidence", Value::F32(self.confidence)),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ExtractedData {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorType {
    Network,
    Parsing,
    Selector,
    Timeout,
    RateLimit,
    Authentication,
    Unknown,
}

impl ComponentType for ErrorType {
    fn ty() -> ValueType {
        ValueType::Enum(EnumType::new(None, [
            "network",
            "parsing",
            "selector",
            "timeout",
            "rate-limit",
            "authentication",
            "unknown",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Enum(enum_val) = value {
            let discriminant = enum_val.discriminant();
            match discriminant {
                0 => Ok(ErrorType::Network),
                1 => Ok(ErrorType::Parsing),
                2 => Ok(ErrorType::Selector),
                3 => Ok(ErrorType::Timeout),
                4 => Ok(ErrorType::RateLimit),
                5 => Ok(ErrorType::Authentication),
                6 => Ok(ErrorType::Unknown),
                _ => bail!("Invalid enum discriminant: {}", discriminant),
            }
        } else {
            bail!("Expected Enum value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let enum_type = EnumType::new(None, [
            "network",
            "parsing",
            "selector",
            "timeout",
            "rate-limit",
            "authentication",
            "unknown",
        ]).unwrap();

        let discriminant = match self {
            ErrorType::Network => 0,
            ErrorType::Parsing => 1,
            ErrorType::Selector => 2,
            ErrorType::Timeout => 3,
            ErrorType::RateLimit => 4,
            ErrorType::Authentication => 5,
            ErrorType::Unknown => 6,
        };

        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))
    }
}

impl UnaryComponentType for ErrorType {}

#[derive(Debug, Clone)]
pub struct ScrapeError {
    pub error_type: ErrorType,
    pub message: String,
    pub url: Option<String>,
    pub timestamp: u64,
    pub recoverable: bool,
}

impl ComponentType for ScrapeError {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("error-type", ErrorType::ty()),
                    ("message", ValueType::String),
                    ("url", ValueType::Option(OptionType::new(ValueType::String))),
                    ("timestamp", ValueType::U64),
                    ("recoverable", ValueType::Bool),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let error_type = record
                .field("error-type")
                .ok_or_else(|| anyhow!("Missing 'error-type' field"))?;
            let message = record
                .field("message")
                .ok_or_else(|| anyhow!("Missing 'message' field"))?;
            let url = record
                .field("url")
                .ok_or_else(|| anyhow!("Missing 'url' field"))?;
            let timestamp = record
                .field("timestamp")
                .ok_or_else(|| anyhow!("Missing 'timestamp' field"))?;
            let recoverable = record
                .field("recoverable")
                .ok_or_else(|| anyhow!("Missing 'recoverable' field"))?;

            let error_type = ErrorType::from_value(&error_type)?;
            let message = if let Value::String(s) = message { s.to_string() } else { bail!("Expected string") };
            let url = Option::<String>::from_value(&url)?;
            let timestamp = if let Value::U64(x) = timestamp { x } else { bail!("Expected u64") };
            let recoverable = if let Value::Bool(x) = recoverable { x } else { bail!("Expected bool") };

            Ok(ScrapeError {
                error_type,
                message,
                url,
                timestamp,
                recoverable,
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
                    ("error-type", ErrorType::ty()),
                    ("message", ValueType::String),
                    ("url", ValueType::Option(OptionType::new(ValueType::String))),
                    ("timestamp", ValueType::U64),
                    ("recoverable", ValueType::Bool),
                ],
            ).unwrap(),
            [
                ("error-type", self.error_type.into_value()?),
                ("message", Value::String(self.message.into())),
                ("url", self.url.into_value()?),
                ("timestamp", Value::U64(self.timestamp)),
                ("recoverable", Value::Bool(self.recoverable)),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ScrapeError {}

#[derive(Debug, Clone)]
pub struct ScrapeMetadata {
    pub start_time: u64,
    pub end_time: u64,
    pub duration_ms: u32,
    pub pages_visited: u32,
    pub elements_extracted: u32,
    pub cache_hits: u32,
    pub retry_count: u32,
}

impl ComponentType for ScrapeMetadata {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("start-time", ValueType::U64),
                    ("end-time", ValueType::U64),
                    ("duration-ms", ValueType::U32),
                    ("pages-visited", ValueType::U32),
                    ("elements-extracted", ValueType::U32),
                    ("cache-hits", ValueType::U32),
                    ("retry-count", ValueType::U32),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let start_time = record
                .field("start-time")
                .ok_or_else(|| anyhow!("Missing 'start-time' field"))?;
            let end_time = record
                .field("end-time")
                .ok_or_else(|| anyhow!("Missing 'end-time' field"))?;
            let duration_ms = record
                .field("duration-ms")
                .ok_or_else(|| anyhow!("Missing 'duration-ms' field"))?;
            let pages_visited = record
                .field("pages-visited")
                .ok_or_else(|| anyhow!("Missing 'pages-visited' field"))?;
            let elements_extracted = record
                .field("elements-extracted")
                .ok_or_else(|| anyhow!("Missing 'elements-extracted' field"))?;
            let cache_hits = record
                .field("cache-hits")
                .ok_or_else(|| anyhow!("Missing 'cache-hits' field"))?;
            let retry_count = record
                .field("retry-count")
                .ok_or_else(|| anyhow!("Missing 'retry-count' field"))?;

            let start_time = if let Value::U64(x) = start_time { x } else { bail!("Expected u64") };
            let end_time = if let Value::U64(x) = end_time { x } else { bail!("Expected u64") };
            let duration_ms = if let Value::U32(x) = duration_ms { x } else { bail!("Expected u32") };
            let pages_visited = if let Value::U32(x) = pages_visited { x } else { bail!("Expected u32") };
            let elements_extracted = if let Value::U32(x) = elements_extracted { x } else { bail!("Expected u32") };
            let cache_hits = if let Value::U32(x) = cache_hits { x } else { bail!("Expected u32") };
            let retry_count = if let Value::U32(x) = retry_count { x } else { bail!("Expected u32") };

            Ok(ScrapeMetadata {
                start_time,
                end_time,
                duration_ms,
                pages_visited,
                elements_extracted,
                cache_hits,
                retry_count,
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
                    ("start-time", ValueType::U64),
                    ("end-time", ValueType::U64),
                    ("duration-ms", ValueType::U32),
                    ("pages-visited", ValueType::U32),
                    ("elements-extracted", ValueType::U32),
                    ("cache-hits", ValueType::U32),
                    ("retry-count", ValueType::U32),
                ],
            ).unwrap(),
            [
                ("start-time", Value::U64(self.start_time)),
                ("end-time", Value::U64(self.end_time)),
                ("duration-ms", Value::U32(self.duration_ms)),
                ("pages-visited", Value::U32(self.pages_visited)),
                ("elements-extracted", Value::U32(self.elements_extracted)),
                ("cache-hits", Value::U32(self.cache_hits)),
                ("retry-count", Value::U32(self.retry_count)),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ScrapeMetadata {}



#[derive(Debug, Clone)]
pub struct ScrapingResult {
    pub target: ScrapeTarget,
    pub data: Vec<ExtractedData>,
    pub errors: Vec<ScrapeError>,
    pub metadata: ScrapeMetadata,
    pub has_related_results: bool,
}

impl ComponentType for ScrapingResult {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("target", ScrapeTarget::ty()),
                    ("data", ValueType::List(ListType::new(ExtractedData::ty()))),
                    ("errors", ValueType::List(ListType::new(ScrapeError::ty()))),
                    ("metadata", ScrapeMetadata::ty()),
                    ("has-related-results", ValueType::Bool),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let target = record
                .field("target")
                .ok_or_else(|| anyhow!("Missing 'target' field"))?;
            let data = record
                .field("data")
                .ok_or_else(|| anyhow!("Missing 'data' field"))?;
            let errors = record
                .field("errors")
                .ok_or_else(|| anyhow!("Missing 'errors' field"))?;
            let metadata = record
                .field("metadata")
                .ok_or_else(|| anyhow!("Missing 'metadata' field"))?;
            let has_related_results = record
                .field("has-related-results")
                .ok_or_else(|| anyhow!("Missing 'has-related-results' field"))?;

            let target = ScrapeTarget::from_value(&target)?;
            let data = Vec::<ExtractedData>::from_value(&data)?;
            let errors = Vec::<ScrapeError>::from_value(&errors)?;
            let metadata = ScrapeMetadata::from_value(&metadata)?;
            let has_related_results = if let Value::Bool(x) = has_related_results { x } else { bail!("Expected bool") };

            Ok(ScrapingResult {
                target,
                data,
                errors,
                metadata,
                has_related_results,
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
                    ("target", ScrapeTarget::ty()),
                    ("data", ValueType::List(ListType::new(ExtractedData::ty()))),
                    ("errors", ValueType::List(ListType::new(ScrapeError::ty()))),
                    ("metadata", ScrapeMetadata::ty()),
                    ("has-related-results", ValueType::Bool),
                ],
            ).unwrap(),
            [
                ("target", self.target.into_value()?),
                ("data", self.data.into_value()?),
                ("errors", self.errors.into_value()?),
                ("metadata", self.metadata.into_value()?),
                ("has-related-results", Value::Bool(self.has_related_results)),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ScrapingResult {}



#[derive(Debug, Clone)]
pub struct ScrapeStatistics {
    pub total_requests: u32,
    pub successful_requests: u32,
    pub failed_requests: u32,
    pub total_data_extracted: u32,
    pub average_response_time_ms: f32,
    pub cache_hit_rate: f32,
    pub error_breakdown: Vec<(ErrorType, u32)>,
}

impl ComponentType for ScrapeStatistics {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("total-requests", ValueType::U32),
                    ("successful-requests", ValueType::U32),
                    ("failed-requests", ValueType::U32),
                    ("total-data-extracted", ValueType::U32),
                    ("average-response-time-ms", ValueType::F32),
                    ("cache-hit-rate", ValueType::F32),
                    ("error-breakdown", ValueType::List(ListType::new(ValueType::Tuple(TupleType::new(None, [ErrorType::ty(), ValueType::U32]))))),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let total_requests = record
                .field("total-requests")
                .ok_or_else(|| anyhow!("Missing 'total-requests' field"))?;
            let successful_requests = record
                .field("successful-requests")
                .ok_or_else(|| anyhow!("Missing 'successful-requests' field"))?;
            let failed_requests = record
                .field("failed-requests")
                .ok_or_else(|| anyhow!("Missing 'failed-requests' field"))?;
            let total_data_extracted = record
                .field("total-data-extracted")
                .ok_or_else(|| anyhow!("Missing 'total-data-extracted' field"))?;
            let average_response_time_ms = record
                .field("average-response-time-ms")
                .ok_or_else(|| anyhow!("Missing 'average-response-time-ms' field"))?;
            let cache_hit_rate = record
                .field("cache-hit-rate")
                .ok_or_else(|| anyhow!("Missing 'cache-hit-rate' field"))?;
            let error_breakdown = record
                .field("error-breakdown")
                .ok_or_else(|| anyhow!("Missing 'error-breakdown' field"))?;

            let total_requests = if let Value::U32(x) = total_requests { x } else { bail!("Expected u32") };
            let successful_requests = if let Value::U32(x) = successful_requests { x } else { bail!("Expected u32") };
            let failed_requests = if let Value::U32(x) = failed_requests { x } else { bail!("Expected u32") };
            let total_data_extracted = if let Value::U32(x) = total_data_extracted { x } else { bail!("Expected u32") };
            let average_response_time_ms = if let Value::F32(x) = average_response_time_ms { x } else { bail!("Expected f32") };
            let cache_hit_rate = if let Value::F32(x) = cache_hit_rate { x } else { bail!("Expected f32") };
            let error_breakdown = Vec::<(ErrorType, u32)>::from_value(&error_breakdown)?;

            Ok(ScrapeStatistics {
                total_requests,
                successful_requests,
                failed_requests,
                total_data_extracted,
                average_response_time_ms,
                cache_hit_rate,
                error_breakdown,
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
                    ("total-requests", ValueType::U32),
                    ("successful-requests", ValueType::U32),
                    ("failed-requests", ValueType::U32),
                    ("total-data-extracted", ValueType::U32),
                    ("average-response-time-ms", ValueType::F32),
                    ("cache-hit-rate", ValueType::F32),
                    ("error-breakdown", ValueType::List(ListType::new(ValueType::Tuple(TupleType::new(None, [ErrorType::ty(), ValueType::U32]))))),
                ],
            ).unwrap(),
            [
                ("total-requests", Value::U32(self.total_requests)),
                ("successful-requests", Value::U32(self.successful_requests)),
                ("failed-requests", Value::U32(self.failed_requests)),
                ("total-data-extracted", Value::U32(self.total_data_extracted)),
                ("average-response-time-ms", Value::F32(self.average_response_time_ms)),
                ("cache-hit-rate", Value::F32(self.cache_hit_rate)),
                ("error-breakdown", self.error_breakdown.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ScrapeStatistics {}

#[derive(Debug, Clone)]
pub enum TransformOperation {
    ExtractText,
    ExtractNumbers,
    ExtractUrls,
    ExtractEmails,
    Trim,
    Lowercase,
    Uppercase,
    Regex(String),
    Custom(String),
}

impl ComponentType for TransformOperation {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("extract-text", None),
                    VariantCase::new("extract-numbers", None),
                    VariantCase::new("extract-urls", None),
                    VariantCase::new("extract-emails", None),
                    VariantCase::new("trim", None),
                    VariantCase::new("lowercase", None),
                    VariantCase::new("uppercase", None),
                    VariantCase::new("regex", Some(ValueType::String)),
                    VariantCase::new("custom", Some(ValueType::String)),
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
                "extract-text" => Ok(TransformOperation::ExtractText),
                "extract-numbers" => Ok(TransformOperation::ExtractNumbers),
                "extract-urls" => Ok(TransformOperation::ExtractUrls),
                "extract-emails" => Ok(TransformOperation::ExtractEmails),
                "trim" => Ok(TransformOperation::Trim),
                "lowercase" => Ok(TransformOperation::Lowercase),
                "uppercase" => Ok(TransformOperation::Uppercase),
                "regex" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(TransformOperation::Regex(converted))
                    } else {
                        bail!("Expected payload for regex case")
                    }
                }
                "custom" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(TransformOperation::Custom(converted))
                    } else {
                        bail!("Expected payload for custom case")
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
                VariantCase::new("extract-text", None),
                VariantCase::new("extract-numbers", None),
                VariantCase::new("extract-urls", None),
                VariantCase::new("extract-emails", None),
                VariantCase::new("trim", None),
                VariantCase::new("lowercase", None),
                VariantCase::new("uppercase", None),
                VariantCase::new("regex", Some(ValueType::String)),
                VariantCase::new("custom", Some(ValueType::String)),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            TransformOperation::ExtractText => (0, None),
            TransformOperation::ExtractNumbers => (1, None),
            TransformOperation::ExtractUrls => (2, None),
            TransformOperation::ExtractEmails => (3, None),
            TransformOperation::Trim => (4, None),
            TransformOperation::Lowercase => (5, None),
            TransformOperation::Uppercase => (6, None),
            TransformOperation::Regex(val) => (7, Some(Value::String(val.into()))),
            TransformOperation::Custom(val) => (8, Some(Value::String(val.into()))),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for TransformOperation {}

#[derive(Debug, Clone)]
pub enum FilterCondition {
    Equals(String),
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    MatchesRegex(String),
    GreaterThan(f64),
    LessThan(f64),
    IsEmpty,
    IsNotEmpty,
}

impl ComponentType for FilterCondition {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("equals", Some(ValueType::String)),
                    VariantCase::new("contains", Some(ValueType::String)),
                    VariantCase::new("starts-with", Some(ValueType::String)),
                    VariantCase::new("ends-with", Some(ValueType::String)),
                    VariantCase::new("matches-regex", Some(ValueType::String)),
                    VariantCase::new("greater-than", Some(ValueType::F64)),
                    VariantCase::new("less-than", Some(ValueType::F64)),
                    VariantCase::new("is-empty", None),
                    VariantCase::new("is-not-empty", None),
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
                "equals" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(FilterCondition::Equals(converted))
                    } else {
                        bail!("Expected payload for equals case")
                    }
                }
                "contains" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(FilterCondition::Contains(converted))
                    } else {
                        bail!("Expected payload for contains case")
                    }
                }
                "starts-with" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(FilterCondition::StartsWith(converted))
                    } else {
                        bail!("Expected payload for starts-with case")
                    }
                }
                "ends-with" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(FilterCondition::EndsWith(converted))
                    } else {
                        bail!("Expected payload for ends-with case")
                    }
                }
                "matches-regex" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(FilterCondition::MatchesRegex(converted))
                    } else {
                        bail!("Expected payload for matches-regex case")
                    }
                }
                "greater-than" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::F64(x) = payload_value { x } else { bail!("Expected f64") };
                        Ok(FilterCondition::GreaterThan(converted))
                    } else {
                        bail!("Expected payload for greater-than case")
                    }
                }
                "less-than" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::F64(x) = payload_value { x } else { bail!("Expected f64") };
                        Ok(FilterCondition::LessThan(converted))
                    } else {
                        bail!("Expected payload for less-than case")
                    }
                }
                "is-empty" => Ok(FilterCondition::IsEmpty),
                "is-not-empty" => Ok(FilterCondition::IsNotEmpty),
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
                VariantCase::new("equals", Some(ValueType::String)),
                VariantCase::new("contains", Some(ValueType::String)),
                VariantCase::new("starts-with", Some(ValueType::String)),
                VariantCase::new("ends-with", Some(ValueType::String)),
                VariantCase::new("matches-regex", Some(ValueType::String)),
                VariantCase::new("greater-than", Some(ValueType::F64)),
                VariantCase::new("less-than", Some(ValueType::F64)),
                VariantCase::new("is-empty", None),
                VariantCase::new("is-not-empty", None),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            FilterCondition::Equals(val) => (0, Some(Value::String(val.into()))),
            FilterCondition::Contains(val) => (1, Some(Value::String(val.into()))),
            FilterCondition::StartsWith(val) => (2, Some(Value::String(val.into()))),
            FilterCondition::EndsWith(val) => (3, Some(Value::String(val.into()))),
            FilterCondition::MatchesRegex(val) => (4, Some(Value::String(val.into()))),
            FilterCondition::GreaterThan(val) => (5, Some(Value::F64(val))),
            FilterCondition::LessThan(val) => (6, Some(Value::F64(val))),
            FilterCondition::IsEmpty => (7, None),
            FilterCondition::IsNotEmpty => (8, None),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for FilterCondition {}

#[derive(Debug, Clone)]
pub struct FilterRule {
    pub field: String,
    pub condition: FilterCondition,
}

impl ComponentType for FilterRule {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("field", ValueType::String),
                    ("condition", FilterCondition::ty()),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let field = record
                .field("field")
                .ok_or_else(|| anyhow!("Missing 'field' field"))?;
            let condition = record
                .field("condition")
                .ok_or_else(|| anyhow!("Missing 'condition' field"))?;

            let field = if let Value::String(s) = field { s.to_string() } else { bail!("Expected string") };
            let condition = FilterCondition::from_value(&condition)?;

            Ok(FilterRule {
                field,
                condition,
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
                    ("field", ValueType::String),
                    ("condition", FilterCondition::ty()),
                ],
            ).unwrap(),
            [
                ("field", Value::String(self.field.into())),
                ("condition", self.condition.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for FilterRule {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationType {
    Required,
    Email,
    Url,
    Numeric,
    Date,
    LengthMin,
    LengthMax,
    Pattern,
}

impl ComponentType for ValidationType {
    fn ty() -> ValueType {
        ValueType::Enum(EnumType::new(None, [
            "required",
            "email",
            "url",
            "numeric",
            "date",
            "length-min",
            "length-max",
            "pattern",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Enum(enum_val) = value {
            let discriminant = enum_val.discriminant();
            match discriminant {
                0 => Ok(ValidationType::Required),
                1 => Ok(ValidationType::Email),
                2 => Ok(ValidationType::Url),
                3 => Ok(ValidationType::Numeric),
                4 => Ok(ValidationType::Date),
                5 => Ok(ValidationType::LengthMin),
                6 => Ok(ValidationType::LengthMax),
                7 => Ok(ValidationType::Pattern),
                _ => bail!("Invalid enum discriminant: {}", discriminant),
            }
        } else {
            bail!("Expected Enum value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let enum_type = EnumType::new(None, [
            "required",
            "email",
            "url",
            "numeric",
            "date",
            "length-min",
            "length-max",
            "pattern",
        ]).unwrap();

        let discriminant = match self {
            ValidationType::Required => 0,
            ValidationType::Email => 1,
            ValidationType::Url => 2,
            ValidationType::Numeric => 3,
            ValidationType::Date => 4,
            ValidationType::LengthMin => 5,
            ValidationType::LengthMax => 6,
            ValidationType::Pattern => 7,
        };

        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))
    }
}

impl UnaryComponentType for ValidationType {}

#[derive(Debug, Clone)]
pub struct ValidationRule {
    pub field: String,
    pub rule_type: ValidationType,
    pub error_message: String,
}

impl ComponentType for ValidationRule {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("field", ValueType::String),
                    ("rule-type", ValidationType::ty()),
                    ("error-message", ValueType::String),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let field = record
                .field("field")
                .ok_or_else(|| anyhow!("Missing 'field' field"))?;
            let rule_type = record
                .field("rule-type")
                .ok_or_else(|| anyhow!("Missing 'rule-type' field"))?;
            let error_message = record
                .field("error-message")
                .ok_or_else(|| anyhow!("Missing 'error-message' field"))?;

            let field = if let Value::String(s) = field { s.to_string() } else { bail!("Expected string") };
            let rule_type = ValidationType::from_value(&rule_type)?;
            let error_message = if let Value::String(s) = error_message { s.to_string() } else { bail!("Expected string") };

            Ok(ValidationRule {
                field,
                rule_type,
                error_message,
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
                    ("field", ValueType::String),
                    ("rule-type", ValidationType::ty()),
                    ("error-message", ValueType::String),
                ],
            ).unwrap(),
            [
                ("field", Value::String(self.field.into())),
                ("rule-type", self.rule_type.into_value()?),
                ("error-message", Value::String(self.error_message.into())),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ValidationRule {}




#[derive(Debug, Clone)]
pub struct PipelineStage {
    pub name: String,
    pub transforms: Vec<TransformOperation>,
    pub filters: Vec<FilterRule>,
    pub validators: Vec<ValidationRule>,
}

impl ComponentType for PipelineStage {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("name", ValueType::String),
                    ("transforms", ValueType::List(ListType::new(TransformOperation::ty()))),
                    ("filters", ValueType::List(ListType::new(FilterRule::ty()))),
                    ("validators", ValueType::List(ListType::new(ValidationRule::ty()))),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let name = record
                .field("name")
                .ok_or_else(|| anyhow!("Missing 'name' field"))?;
            let transforms = record
                .field("transforms")
                .ok_or_else(|| anyhow!("Missing 'transforms' field"))?;
            let filters = record
                .field("filters")
                .ok_or_else(|| anyhow!("Missing 'filters' field"))?;
            let validators = record
                .field("validators")
                .ok_or_else(|| anyhow!("Missing 'validators' field"))?;

            let name = if let Value::String(s) = name { s.to_string() } else { bail!("Expected string") };
            let transforms = Vec::<TransformOperation>::from_value(&transforms)?;
            let filters = Vec::<FilterRule>::from_value(&filters)?;
            let validators = Vec::<ValidationRule>::from_value(&validators)?;

            Ok(PipelineStage {
                name,
                transforms,
                filters,
                validators,
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
                    ("name", ValueType::String),
                    ("transforms", ValueType::List(ListType::new(TransformOperation::ty()))),
                    ("filters", ValueType::List(ListType::new(FilterRule::ty()))),
                    ("validators", ValueType::List(ListType::new(ValidationRule::ty()))),
                ],
            ).unwrap(),
            [
                ("name", Value::String(self.name.into())),
                ("transforms", self.transforms.into_value()?),
                ("filters", self.filters.into_value()?),
                ("validators", self.validators.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for PipelineStage {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorHandlingStrategy {
    FailFast,
    SkipErrors,
    CollectErrors,
    RetryFailed,
}

impl ComponentType for ErrorHandlingStrategy {
    fn ty() -> ValueType {
        ValueType::Enum(EnumType::new(None, [
            "fail-fast",
            "skip-errors",
            "collect-errors",
            "retry-failed",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Enum(enum_val) = value {
            let discriminant = enum_val.discriminant();
            match discriminant {
                0 => Ok(ErrorHandlingStrategy::FailFast),
                1 => Ok(ErrorHandlingStrategy::SkipErrors),
                2 => Ok(ErrorHandlingStrategy::CollectErrors),
                3 => Ok(ErrorHandlingStrategy::RetryFailed),
                _ => bail!("Invalid enum discriminant: {}", discriminant),
            }
        } else {
            bail!("Expected Enum value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let enum_type = EnumType::new(None, [
            "fail-fast",
            "skip-errors",
            "collect-errors",
            "retry-failed",
        ]).unwrap();

        let discriminant = match self {
            ErrorHandlingStrategy::FailFast => 0,
            ErrorHandlingStrategy::SkipErrors => 1,
            ErrorHandlingStrategy::CollectErrors => 2,
            ErrorHandlingStrategy::RetryFailed => 3,
        };

        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))
    }
}

impl UnaryComponentType for ErrorHandlingStrategy {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Csv,
    Xml,
    Custom,
}

impl ComponentType for OutputFormat {
    fn ty() -> ValueType {
        ValueType::Enum(EnumType::new(None, [
            "json",
            "csv",
            "xml",
            "custom",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Enum(enum_val) = value {
            let discriminant = enum_val.discriminant();
            match discriminant {
                0 => Ok(OutputFormat::Json),
                1 => Ok(OutputFormat::Csv),
                2 => Ok(OutputFormat::Xml),
                3 => Ok(OutputFormat::Custom),
                _ => bail!("Invalid enum discriminant: {}", discriminant),
            }
        } else {
            bail!("Expected Enum value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let enum_type = EnumType::new(None, [
            "json",
            "csv",
            "xml",
            "custom",
        ]).unwrap();

        let discriminant = match self {
            OutputFormat::Json => 0,
            OutputFormat::Csv => 1,
            OutputFormat::Xml => 2,
            OutputFormat::Custom => 3,
        };

        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))
    }
}

impl UnaryComponentType for OutputFormat {}


#[derive(Debug, Clone)]
pub struct DataPipeline {
    pub name: String,
    pub stages: Vec<PipelineStage>,
    pub error_handling: ErrorHandlingStrategy,
    pub output_format: OutputFormat,
}

impl ComponentType for DataPipeline {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("name", ValueType::String),
                    ("stages", ValueType::List(ListType::new(PipelineStage::ty()))),
                    ("error-handling", ErrorHandlingStrategy::ty()),
                    ("output-format", OutputFormat::ty()),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let name = record
                .field("name")
                .ok_or_else(|| anyhow!("Missing 'name' field"))?;
            let stages = record
                .field("stages")
                .ok_or_else(|| anyhow!("Missing 'stages' field"))?;
            let error_handling = record
                .field("error-handling")
                .ok_or_else(|| anyhow!("Missing 'error-handling' field"))?;
            let output_format = record
                .field("output-format")
                .ok_or_else(|| anyhow!("Missing 'output-format' field"))?;

            let name = if let Value::String(s) = name { s.to_string() } else { bail!("Expected string") };
            let stages = Vec::<PipelineStage>::from_value(&stages)?;
            let error_handling = ErrorHandlingStrategy::from_value(&error_handling)?;
            let output_format = OutputFormat::from_value(&output_format)?;

            Ok(DataPipeline {
                name,
                stages,
                error_handling,
                output_format,
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
                    ("name", ValueType::String),
                    ("stages", ValueType::List(ListType::new(PipelineStage::ty()))),
                    ("error-handling", ErrorHandlingStrategy::ty()),
                    ("output-format", OutputFormat::ty()),
                ],
            ).unwrap(),
            [
                ("name", Value::String(self.name.into())),
                ("stages", self.stages.into_value()?),
                ("error-handling", self.error_handling.into_value()?),
                ("output-format", self.output_format.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for DataPipeline {}


















// ========== Host Imports ==========

/// Host trait for interface: example:webscraper/http
pub trait HttpHost {
    fn make_request(&mut self, request: HttpRequest) -> Result<HttpResponse, String>;
    fn log_info(&mut self, message: String) -> ();
    fn log_error(&mut self, message: String) -> ();
}

/// Host trait for interface: example:webscraper/dom
pub trait DomHost {
    fn parse_html(&mut self, html: String) -> Result<Vec<DomElement>, String>;
    fn query_selector(&mut self, root: DomElement, selector: Selector) -> Result<Vec<DomElement>, String>;
}

/// Host trait for interface: example:webscraper/scraper
pub trait ScraperHost {
}

/// Host trait for interface: example:webscraper/pipeline
pub trait PipelineHost {
}

pub mod imports {
    use super::*;

    pub fn register_http_host<T: HttpHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        store: &mut Store<T, E>,
    ) -> Result<()> {
        let host_interface = linker
            .define_instance("example:webscraper/http".try_into().unwrap())
            .context("Failed to define host interface")?;

        host_interface
            .define_func(
                "make-request",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [HttpRequest::ty(), ],
                        [ValueType::Result(ResultType::new(Some(HttpResponse::ty()), Some(ValueType::String)))],
                    ),
                    |mut ctx, params, results| {
                        let request = HttpRequest::from_value(&params[0])?;
                        let result = ctx.data_mut().make_request(request);
                        results[0] = result.into_value()?;
                        Ok(())
                    },
                ),
            )
            .context("Failed to define make-request function")?;

        host_interface
            .define_func(
                "log-info",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::String, ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let message = if let Value::String(s) = &params[0] { s.to_string() } else { bail!("Expected string") };
                        ctx.data_mut().log_info(message);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define log-info function")?;

        host_interface
            .define_func(
                "log-error",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::String, ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let message = if let Value::String(s) = &params[0] { s.to_string() } else { bail!("Expected string") };
                        ctx.data_mut().log_error(message);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define log-error function")?;

        Ok(())
    }

    pub fn register_dom_host<T: DomHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        store: &mut Store<T, E>,
    ) -> Result<()> {
        let host_interface = linker
            .define_instance("example:webscraper/dom".try_into().unwrap())
            .context("Failed to define host interface")?;

        host_interface
            .define_func(
                "parse-html",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::String, ],
                        [ValueType::Result(ResultType::new(Some(ValueType::List(ListType::new(DomElement::ty()))), Some(ValueType::String)))],
                    ),
                    |mut ctx, params, results| {
                        let html = if let Value::String(s) = &params[0] { s.to_string() } else { bail!("Expected string") };
                        let result = ctx.data_mut().parse_html(html);
                        results[0] = result.into_value()?;
                        Ok(())
                    },
                ),
            )
            .context("Failed to define parse-html function")?;

        host_interface
            .define_func(
                "query-selector",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [DomElement::ty(), Selector::ty(), ],
                        [ValueType::Result(ResultType::new(Some(ValueType::List(ListType::new(DomElement::ty()))), Some(ValueType::String)))],
                    ),
                    |mut ctx, params, results| {
                        let root = DomElement::from_value(&params[0])?;
                        let selector = Selector::from_value(&params[1])?;
                        let result = ctx.data_mut().query_selector(root, selector);
                        results[0] = result.into_value()?;
                        Ok(())
                    },
                ),
            )
            .context("Failed to define query-selector function")?;

        Ok(())
    }

    pub fn register_scraper_host<T: ScraperHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        _store: &mut Store<T, E>,
    ) -> Result<()> {
        let _host_interface = linker
            .define_instance("example:webscraper/scraper".try_into().unwrap())
            .context("Failed to define host interface")?;

        Ok(())
    }

    pub fn register_pipeline_host<T: PipelineHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        _store: &mut Store<T, E>,
    ) -> Result<()> {
        let _host_interface = linker
            .define_instance("example:webscraper/pipeline".try_into().unwrap())
            .context("Failed to define host interface")?;

        Ok(())
    }

}

// ========== Guest Exports ==========

pub mod exports_exports {
    use super::*;

    pub const INTERFACE_NAME: &str = "example:webscraper/exports";

    #[allow(clippy::type_complexity)]
    pub fn get_scrape_website<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(ScrapeTarget, Option<DataPipeline>, Vec<HttpHeader>), Result<ScrapingResult, String>>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("scrape-website")
            .ok_or_else(|| anyhow!("Function 'scrape-website' not found"))?
            .typed::<(ScrapeTarget, Option<DataPipeline>, Vec<HttpHeader>), Result<ScrapingResult, String>>()
    }

    #[allow(clippy::type_complexity)]
    pub fn get_scrape_batch<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(Vec<ScrapeTarget>, Option<DataPipeline>), Result<Vec<Result<ScrapingResult, String>>, String>>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("scrape-batch")
            .ok_or_else(|| anyhow!("Function 'scrape-batch' not found"))?
            .typed::<(Vec<ScrapeTarget>, Option<DataPipeline>), Result<Vec<Result<ScrapingResult, String>>, String>>()
    }

    #[allow(clippy::type_complexity)]
    pub fn get_get_statistics<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(), ScrapeStatistics>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("get-statistics")
            .ok_or_else(|| anyhow!("Function 'get-statistics' not found"))?
            .typed::<(), ScrapeStatistics>()
    }

    #[allow(clippy::type_complexity)]
    pub fn get_process_data<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(Vec<ExtractedData>, DataPipeline), Result<Vec<ExtractedData>, String>>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("process-data")
            .ok_or_else(|| anyhow!("Function 'process-data' not found"))?
            .typed::<(Vec<ExtractedData>, DataPipeline), Result<Vec<ExtractedData>, String>>()
    }

    #[allow(clippy::type_complexity)]
    pub fn get_transform_response<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(HttpResponse, Vec<Selector>), Result<Vec<ExtractedData>, String>>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("transform-response")
            .ok_or_else(|| anyhow!("Function 'transform-response' not found"))?
            .typed::<(HttpResponse, Vec<Selector>), Result<Vec<ExtractedData>, String>>()
    }

}


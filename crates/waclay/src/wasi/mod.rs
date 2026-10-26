//! WASI support: P1 (core `wasi_snapshot_preview1`) and P2/P3-style
//! component interfaces (`wasi:cli`, `wasi:io`, `wasi:clocks`, `wasi:random`).
//!
//! The host state lives in [`WasiCtx`], placed in the store as user data:
//!
//! ```no_run
//! use waclay::{Engine, Store, Linker};
//! use waclay::wasi::WasiCtx;
//!
//! # fn demo(engine: &Engine) -> waclay::Result<()> {
//! let ctx = WasiCtx::builder()
//!     .arg("hello.wasm")
//!     .env("KEY", "value")
//!     .build();
//! let mut store = Store::new(engine, ctx);
//! let mut linker = Linker::new();
//! waclay::wasi::add_p1_to_linker(&mut linker);   // core modules
//! waclay::wasi::add_to_linker(&mut linker, "0.2.0"); // components
//! # Ok(())
//! # }
//! ```

use crate::component::types::{Handle, Val};
use crate::engine::{CallerView, Extern, Val as CoreVal};
use crate::linker::{HostCtx, Linker};
use crate::resource::SharedTables;
use crate::{Error, Result};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::Instant;

// ---------------------------------------------------------------------------
// WASI errno (preview1)
// ---------------------------------------------------------------------------

#[allow(dead_code)] // full P3 errno surface (used as WASI grows)
const ERRNO_SUCCESS: i32 = 0;
const ERRNO_EBADF: i32 = 8;
#[allow(dead_code)]
const ERRNO_EINVAL: i32 = 28;
#[allow(dead_code)]
const ERRNO_EIO: i32 = 29;
const ERRNO_ENOSYS: i32 = 52;

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// Host state for WASI. Put it in the store as user data.
pub struct WasiCtx {
    /// Command-line arguments (`argv[0]` first).
    pub args: Vec<String>,
    /// Environment variables.
    pub env: Vec<(String, String)>,
    /// Standard input.
    pub stdin: Box<dyn Read + Send>,
    /// Standard output.
    pub stdout: Box<dyn Write + Send>,
    /// Standard error.
    pub stderr: Box<dyn Write + Send>,
    /// Exit code captured from `proc_exit` (P1) or `cli/exit` (P2).
    pub exit_code: Option<i32>,
    pub(crate) start: Instant,
    rng: u64,
    /// rep -> message for `wasi:io/error` resources
    errors: HashMap<u64, String>,
    next_rep: u64,
    /// bytes produced by the deterministic RNG so far (tests)
    pub random_bytes_drawn: usize,
}

impl Default for WasiCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl WasiCtx {
    /// Fresh context inheriting `std::io` handles.
    pub fn new() -> WasiCtx {
        WasiCtx {
            args: Vec::new(),
            env: Vec::new(),
            stdin: Box::new(std::io::empty()),
            stdout: Box::new(std::io::stdout()),
            stderr: Box::new(std::io::stderr()),
            exit_code: None,
            start: Instant::now(),
            rng: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E3779B97F4A7C15)
                | 1,
            errors: HashMap::new(),
            next_rep: 100,
            random_bytes_drawn: 0,
        }
    }

    /// Builder entry point.
    pub fn builder() -> WasiCtxBuilder {
        WasiCtxBuilder::default()
    }

    /// Monotonic nanoseconds since instantiation.
    pub fn monotonic_nanos(&self) -> u64 {
        self.start.elapsed().as_nanos() as u64
    }

    /// Wall clock (seconds, nanoseconds) since the unix epoch.
    pub fn wall_clock(&self) -> (u64, u32) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        (now.as_secs(), now.subsec_nanos())
    }

    /// Fill `buf` with pseudo-random bytes (xorshift*; deterministic per ctx).
    pub fn random_fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let mut x = self.rng;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.rng = x;
            let bytes = (x.wrapping_mul(0x2545F4914F6CDD1D)).to_le_bytes();
            let n = chunk.len();
            chunk.copy_from_slice(&bytes[..n]);
        }
        self.random_bytes_drawn += buf.len();
    }

    /// Get one random u64.
    pub fn random_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        self.random_fill(&mut b);
        u64::from_le_bytes(b)
    }

    fn new_error(&mut self, msg: String) -> u64 {
        let rep = self.next_rep;
        self.next_rep += 1;
        self.errors.insert(rep, msg);
        rep
    }

    fn error_message(&self, rep: u64) -> String {
        self.errors
            .get(&rep)
            .cloned()
            .unwrap_or_else(|| format!("error #{rep}"))
    }
}

/// Builder for [`WasiCtx`].
#[derive(Default)]
pub struct WasiCtxBuilder {
    ctx: Option<WasiCtx>,
}

impl WasiCtxBuilder {
    /// Add a command-line argument.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.ctx_mut().args.push(arg.into());
        self
    }

    /// Add an environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.ctx_mut().env.push((key.into(), value.into()));
        self
    }

    /// Replace stdin.
    pub fn stdin(mut self, r: Box<dyn Read + Send>) -> Self {
        self.ctx_mut().stdin = r;
        self
    }

    /// Replace stdout.
    pub fn stdout(mut self, w: Box<dyn Write + Send>) -> Self {
        self.ctx_mut().stdout = w;
        self
    }

    /// Replace stderr.
    pub fn stderr(mut self, w: Box<dyn Write + Send>) -> Self {
        self.ctx_mut().stderr = w;
        self
    }

    /// Inherit stdin/stdout/stderr from the host process.
    pub fn inherit_stdio(mut self) -> Self {
        let c = self.ctx_mut();
        c.stdin = Box::new(std::io::stdin());
        c.stdout = Box::new(std::io::stdout());
        c.stderr = Box::new(std::io::stderr());
        self
    }

    fn ctx_mut(&mut self) -> &mut WasiCtx {
        self.ctx.get_or_insert_with(WasiCtx::new)
    }

    /// Finish building.
    pub fn build(self) -> WasiCtx {
        self.ctx.unwrap_or_default()
    }
}

fn wasi_ctx(caller: &mut dyn CallerView) -> Result<&mut WasiCtx> {
    caller.data_mut().downcast_mut::<WasiCtx>().ok_or_else(|| {
        Error::Host(
            "store user data is not WasiCtx; construct the store with Store::new(engine, WasiCtx)"
                .into(),
        )
    })
}

// ---------------------------------------------------------------------------
// P1: wasi_snapshot_preview1 (core modules)
// ---------------------------------------------------------------------------

fn caller_memory(caller: &mut dyn CallerView) -> Result<crate::engine::Memory> {
    match caller.get_export("memory") {
        Some(Extern::Memory(m)) => Ok(m),
        _ => Err(Error::trap("guest does not export `memory`")),
    }
}

fn write_u32(
    caller: &mut dyn CallerView,
    mem: &crate::engine::Memory,
    addr: u32,
    v: u32,
) -> Result<()> {
    caller.memory_write(mem, addr as usize, &v.to_le_bytes())
}

fn write_u64(
    caller: &mut dyn CallerView,
    mem: &crate::engine::Memory,
    addr: u32,
    v: u64,
) -> Result<()> {
    caller.memory_write(mem, addr as usize, &v.to_le_bytes())
}

fn read_u32(caller: &mut dyn CallerView, mem: &crate::engine::Memory, addr: u32) -> Result<u32> {
    let mut b = [0u8; 4];
    caller.memory_read(mem, addr as usize, &mut b)?;
    Ok(u32::from_le_bytes(b))
}

#[allow(dead_code)] // reserved for argv/env P3 paths
fn read_cstrs(
    caller: &mut dyn CallerView,
    mem: &crate::engine::Memory,
    ptr: u32,
    len: usize,
) -> Result<Vec<String>> {
    let mut buf = vec![0u8; len];
    caller.memory_read(mem, ptr as usize, &mut buf)?;
    Ok(buf
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect())
}

fn sizes_of(strings: &[(String, String)]) -> (u32, u32) {
    let encoded: Vec<String> = strings.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let count = encoded.len() as u32;
    let bytes = encoded.iter().map(|s| s.len() + 1).sum::<usize>() as u32;
    (count, bytes)
}

fn write_strings(
    caller: &mut dyn CallerView,
    mem: &crate::engine::Memory,
    strings: &[String],
    argv_ptr: u32,
    argv_buf_ptr: u32,
) -> Result<()> {
    let mut buf_ptr = argv_buf_ptr;
    for (i, s) in strings.iter().enumerate() {
        // pointer table entry
        write_u32(caller, mem, argv_ptr + (i as u32) * 4, buf_ptr)?;
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0);
        caller.memory_write(mem, buf_ptr as usize, &bytes)?;
        buf_ptr += bytes.len() as u32;
    }
    Ok(())
}

/// Register all `wasi_snapshot_preview1` functions as raw core imports.
pub fn add_p1_to_linker(linker: &mut Linker) {
    const M: &str = "wasi_snapshot_preview1";

    // -- args ---------------------------------------------------------------
    linker.raw_func(
        M,
        "args_sizes_get",
        Box::new(|caller, args, results| {
            let mem = caller_memory(caller)?;
            let strings: Vec<String> = wasi_ctx(caller)?.args.clone();
            let (count, bytes) = sizes_of(
                &strings
                    .into_iter()
                    .map(|s| (s, String::new()))
                    .collect::<Vec<_>>(),
            );
            write_u32(caller, &mem, args[0].i32()? as u32, count)?;
            write_u32(caller, &mem, args[1].i32()? as u32, bytes)?;
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "args_get",
        Box::new(|caller, args, results| {
            let mem = caller_memory(caller)?;
            let strings = wasi_ctx(caller)?.args.clone();
            write_strings(
                caller,
                &mem,
                &strings,
                args[0].i32()? as u32,
                args[1].i32()? as u32,
            )?;
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );

    // -- environ ------------------------------------------------------------
    linker.raw_func(
        M,
        "environ_sizes_get",
        Box::new(|caller, args, results| {
            let mem = caller_memory(caller)?;
            let env = wasi_ctx(caller)?.env.clone();
            let (count, bytes) = sizes_of(&env);
            write_u32(caller, &mem, args[0].i32()? as u32, count)?;
            write_u32(caller, &mem, args[1].i32()? as u32, bytes)?;
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "environ_get",
        Box::new(|caller, args, results| {
            let mem = caller_memory(caller)?;
            let env = wasi_ctx(caller)?.env.clone();
            let strings: Vec<String> = env.into_iter().map(|(k, v)| format!("{k}={v}")).collect();
            write_strings(
                caller,
                &mem,
                &strings,
                args[0].i32()? as u32,
                args[1].i32()? as u32,
            )?;
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );

    // -- clocks -------------------------------------------------------------
    linker.raw_func(
        M,
        "clock_time_get",
        Box::new(|caller, args, results| {
            let mem = caller_memory(caller)?;
            let id = args[0].i32()?;
            let nanos = match id {
                0 => {
                    wasi_ctx(caller)?.wall_clock().0 * 1_000_000_000
                        + wasi_ctx(caller)?.wall_clock().1 as u64
                }
                1..=3 => wasi_ctx(caller)?.monotonic_nanos(),
                _ => return Err(Error::trap("invalid clock id")),
            };
            write_u64(caller, &mem, args[2].i32()? as u32, nanos)?;
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );

    // -- random -------------------------------------------------------------
    linker.raw_func(
        M,
        "random_get",
        Box::new(|caller, args, results| {
            let mem = caller_memory(caller)?;
            let ptr = args[0].i32()? as u32;
            let len = args[1].i32()? as usize;
            let mut buf = vec![0u8; len];
            wasi_ctx(caller)?.random_fill(&mut buf);
            caller.memory_write(&mem, ptr as usize, &buf)?;
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );

    // -- stdio --------------------------------------------------------------
    linker.raw_func(
        M,
        "fd_write",
        Box::new(|caller, args, results| {
            let mem = caller_memory(caller)?;
            let fd = args[0].i32()?;
            let iovs = args[1].i32()? as u32;
            let iovs_len = args[2].i32()? as u32;
            let nwritten_ptr = args[3].i32()? as u32;
            if fd != 1 && fd != 2 {
                write_u32(caller, &mem, nwritten_ptr, 0)?;
                results.push(CoreVal::I32(ERRNO_EBADF));
                return Ok(());
            }
            let mut total = 0u32;
            for i in 0..iovs_len {
                let base = read_u32(caller, &mem, iovs + i * 8)?;
                let len = read_u32(caller, &mem, iovs + i * 8 + 4)?;
                let mut buf = vec![0u8; len as usize];
                caller.memory_read(&mem, base as usize, &mut buf)?;
                let n = {
                    let ctx = wasi_ctx(caller)?;
                    if fd == 1 {
                        ctx.stdout
                            .write_all(&buf)
                            .map_err(|e| Error::trap(e.to_string()))?;
                    } else {
                        ctx.stderr
                            .write_all(&buf)
                            .map_err(|e| Error::trap(e.to_string()))?;
                    }
                    buf.len() as u32
                };
                total += n;
            }
            write_u32(caller, &mem, nwritten_ptr, total)?;
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "fd_read",
        Box::new(|caller, args, results| {
            let mem = caller_memory(caller)?;
            let fd = args[0].i32()?;
            let iovs = args[1].i32()? as u32;
            let iovs_len = args[2].i32()? as u32;
            let nread_ptr = args[3].i32()? as u32;
            if fd != 0 {
                results.push(CoreVal::I32(ERRNO_EBADF));
                return Ok(());
            }
            let mut total = 0u32;
            for i in 0..iovs_len {
                let base = read_u32(caller, &mem, iovs + i * 8)?;
                let len = read_u32(caller, &mem, iovs + i * 8 + 4)?;
                let mut buf = vec![0u8; len as usize];
                let n = {
                    let ctx = wasi_ctx(caller)?;
                    ctx.stdin
                        .read(&mut buf)
                        .map_err(|e| Error::trap(e.to_string()))?
                };
                buf.truncate(n);
                caller.memory_write(&mem, base as usize, &buf)?;
                total += n as u32;
                if n < buf.capacity() {
                    break;
                }
            }
            write_u32(caller, &mem, nread_ptr, total)?;
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "fd_fdstat_get",
        Box::new(|caller, args, results| {
            let mem = caller_memory(caller)?;
            let fd = args[0].i32()?;
            let buf_ptr = args[1].i32()? as u32;
            if (0..=2).contains(&fd) {
                // fs_filetype = character_device(2)? wasi: 3 = character_device
                let mut stat = [0u8; 24];
                stat[0] = 3;
                caller.memory_write(&mem, buf_ptr as usize, &stat)?;
                results.push(CoreVal::I32(ERRNO_SUCCESS));
            } else {
                results.push(CoreVal::I32(ERRNO_EBADF));
            }
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "fd_close",
        Box::new(|_caller, _args, results| {
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "fd_seek",
        Box::new(|_caller, _args, results| {
            results.push(CoreVal::I32(ERRNO_ENOSYS));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "fd_prestat_get",
        Box::new(|_caller, _args, results| {
            results.push(CoreVal::I32(ERRNO_EBADF));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "fd_prestat_dir_name",
        Box::new(|_caller, _args, results| {
            results.push(CoreVal::I32(ERRNO_EBADF));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "fd_filestat_get",
        Box::new(|_caller, _args, results| {
            results.push(CoreVal::I32(ERRNO_ENOSYS));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "path_open",
        Box::new(|_caller, _args, results| {
            results.push(CoreVal::I32(ERRNO_ENOSYS));
            Ok(())
        }),
    );
    linker.raw_func(
        M,
        "proc_exit",
        Box::new(|caller, args, _results| {
            let code = args[0].i32()?;
            wasi_ctx(caller)?.exit_code = Some(code);
            Err(Error::trap(format!("proc_exit({code})")))
        }),
    );
    linker.raw_func(
        M,
        "sched_yield",
        Box::new(|_caller, _args, results| {
            results.push(CoreVal::I32(ERRNO_SUCCESS));
            Ok(())
        }),
    );
}

// ---------------------------------------------------------------------------
// P2/P3-style component interfaces
// ---------------------------------------------------------------------------

#[allow(dead_code)] // reserved for P2/P3 component WASI
fn resource_rep_of(ctx: &mut HostCtx<'_>, handle: &Val, resource_id: u32) -> Result<u64> {
    match handle {
        Val::Handle(Handle::Borrow { resource, index })
        | Val::Handle(Handle::Own { resource, index }) => {
            let _ = resource;
            ctx.tables.lock().unwrap().resource_rep(resource_id, *index)
        }
        other => Err(Error::value(format!(
            "expected resource handle, found {}",
            other.type_name()
        ))),
    }
}

fn std_stream_id(rep: u64) -> u64 {
    rep // 1 = stdin, 2 = stdout, 3 = stderr
}

/// Register the component-model WASI interfaces under `version`
/// (e.g. `"0.2.0"`, or `"0.3.0"` for WASI P3 builds).
pub fn add_to_linker(linker: &mut Linker, version: &str) {
    let v = version.to_string();

    // --- wasi:random/random -------------------------------------------------
    let key = format!("wasi:random/random@{v}");
    linker.func(
        Some(&key),
        "get-random-u64",
        |ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            let n = wasi_ctx(ctx.caller)?.random_u64();
            out.push(Val::U64(n));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "get-random-bytes",
        |ctx: &mut HostCtx<'_>, args: &[Val], out: &mut Vec<Val>| {
            let len = match &args[0] {
                Val::U64(n) => *n as usize,
                other => {
                    return Err(Error::value(format!(
                        "expected u64 len, got {}",
                        other.type_name()
                    )))
                }
            };
            let mut buf = vec![0u8; len];
            wasi_ctx(ctx.caller)?.random_fill(&mut buf);
            out.push(Val::List(buf.into_iter().map(Val::U8).collect()));
            Ok(())
        },
    );

    // --- wasi:clocks/wall-clock --------------------------------------------
    let key = format!("wasi:clocks/wall-clock@{v}");
    linker.func(
        Some(&key),
        "now",
        |ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            let (s, ns) = wasi_ctx(ctx.caller)?.wall_clock();
            out.push(Val::Record(vec![
                ("seconds".into(), Val::U64(s)),
                ("nanoseconds".into(), Val::U32(ns)),
            ]));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "resolution",
        |ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            let _ = wasi_ctx(ctx.caller)?;
            out.push(Val::Record(vec![
                ("seconds".into(), Val::U64(0)),
                ("nanoseconds".into(), Val::U32(1)),
            ]));
            Ok(())
        },
    );

    // --- wasi:clocks/monotonic-clock ---------------------------------------
    let key = format!("wasi:clocks/monotonic-clock@{v}");
    linker.func(
        Some(&key),
        "now",
        |ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            let n = wasi_ctx(ctx.caller)?.monotonic_nanos();
            out.push(Val::U64(n));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "resolution",
        |ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            let _ = wasi_ctx(ctx.caller)?;
            out.push(Val::U64(1));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "subscribe-instant",
        |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            // own<pollable> — rep 0 is an "always ready" pollable
            out.push(Val::Handle(Handle::Own {
                resource: 0,
                index: 0,
            }));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "subscribe-duration",
        |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            out.push(Val::Handle(Handle::Own {
                resource: 0,
                index: 0,
            }));
            Ok(())
        },
    );

    // --- wasi:io/error ------------------------------------------------------
    let key = format!("wasi:io/error@{v}");
    linker.func(
        Some(&key),
        "[constructor]error",
        |ctx: &mut HostCtx<'_>, args: &[Val], out: &mut Vec<Val>| {
            let msg = match args.first() {
                Some(Val::Option(Some(m))) => match m.as_ref() {
                    Val::String(s) => s.clone(),
                    _ => "error".to_string(),
                },
                _ => "error".to_string(),
            };
            let rep = wasi_ctx(ctx.caller)?.new_error(msg);
            // own<error>: host returns the REP; the shim wraps it via resource-new
            out.push(Val::Handle(Handle::Own {
                resource: 0,
                index: rep as u32,
            }));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "[method]error.to-debug-string",
        |ctx: &mut HostCtx<'_>, args: &[Val], out: &mut Vec<Val>| {
            // borrow<error> param: handle index == rep (we never allocate real
            // handles for error resources because the shim's resource-new table
            // maps them 1:1 — resolve through the table first, then fall back)
            let msg = match &args[0] {
                Val::Handle(h) => {
                    let rep = ctx
                        .tables
                        .lock()
                        .unwrap()
                        .resource_rep(usize::MAX as u32, h.index())
                        .or(Ok::<u64, crate::Error>(h.index() as u64));
                    match rep {
                        Ok(r) => wasi_ctx(ctx.caller)?.error_message(r),
                        Err(_) => wasi_ctx(ctx.caller)?.error_message(h.index() as u64),
                    }
                }
                other => format!("error ({})", other.type_name()),
            };
            out.push(Val::String(msg));
            Ok(())
        },
    );

    // --- wasi:io/poll -------------------------------------------------------
    let key = format!("wasi:io/poll@{v}");
    linker.func(
        Some(&key),
        "[method]pollable.ready",
        |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            out.push(Val::Bool(true));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "[method]pollable.block",
        |_ctx: &mut HostCtx<'_>, _args: &[Val], _out: &mut Vec<Val>| Ok(()),
    );

    // --- wasi:io/streams ----------------------------------------------------
    let key = format!("wasi:io/streams@{v}");
    linker.func(
        Some(&key),
        "[method]input-stream.read",
        |ctx: &mut HostCtx<'_>, args: &[Val], out: &mut Vec<Val>| {
            let stream = stream_id_from(ctx, &args[0])?;
            let len = match &args[1] {
                Val::U64(n) => *n as usize,
                other => {
                    return Err(Error::value(format!(
                        "expected u64 len, got {}",
                        other.type_name()
                    )))
                }
            };
            if stream != 1 {
                return Err(Error::trap(
                    "input-stream: only stdin (handle rep 1) is available",
                ));
            }
            let mut buf = vec![0u8; len];
            let n = wasi_ctx(ctx.caller)?
                .stdin
                .read(&mut buf)
                .map_err(|e| Error::trap(e.to_string()))?;
            buf.truncate(n);
            out.push(Val::Result(Ok(Some(Box::new(Val::List(
                buf.into_iter().map(Val::U8).collect(),
            ))))));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "[method]input-stream.blocking-read",
        |ctx: &mut HostCtx<'_>, args: &[Val], out: &mut Vec<Val>| {
            // same as read for our blocking stdio model
            let stream = stream_id_from(ctx, &args[0])?;
            let len = match &args[1] {
                Val::U64(n) => *n as usize,
                other => {
                    return Err(Error::value(format!(
                        "expected u64 len, got {}",
                        other.type_name()
                    )))
                }
            };
            if stream != 1 {
                return Err(Error::trap("input-stream: only stdin is available"));
            }
            let mut buf = vec![0u8; len];
            let n = wasi_ctx(ctx.caller)?
                .stdin
                .read(&mut buf)
                .map_err(|e| Error::trap(e.to_string()))?;
            buf.truncate(n);
            out.push(Val::Result(Ok(Some(Box::new(Val::List(
                buf.into_iter().map(Val::U8).collect(),
            ))))));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "[method]output-stream.write",
        |ctx: &mut HostCtx<'_>, args: &[Val], out: &mut Vec<Val>| {
            let stream = stream_id_from(ctx, &args[0])?;
            let bytes = match &args[1] {
                Val::List(items) => {
                    let mut b = Vec::with_capacity(items.len());
                    for it in items {
                        match it {
                            Val::U8(x) => b.push(*x),
                            other => {
                                return Err(Error::value(format!(
                                    "expected u8, got {}",
                                    other.type_name()
                                )))
                            }
                        }
                    }
                    b
                }
                other => {
                    return Err(Error::value(format!(
                        "expected list<u8>, got {}",
                        other.type_name()
                    )))
                }
            };
            {
                let ctx2 = wasi_ctx(ctx.caller)?;
                match stream {
                    2 => ctx2
                        .stdout
                        .write_all(&bytes)
                        .map_err(|e| Error::trap(e.to_string()))?,
                    3 => ctx2
                        .stderr
                        .write_all(&bytes)
                        .map_err(|e| Error::trap(e.to_string()))?,
                    _ => {
                        return Err(Error::trap(
                            "output-stream: only stdout/stderr are available",
                        ))
                    }
                }
            }
            out.push(Val::Result(Ok(None)));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "[method]output-stream.blocking-write",
        |ctx: &mut HostCtx<'_>, args: &[Val], out: &mut Vec<Val>| {
            // identical semantics for our blocking streams
            let stream = stream_id_from(ctx, &args[0])?;
            let bytes = match &args[1] {
                Val::List(items) => {
                    let mut b = Vec::with_capacity(items.len());
                    for it in items {
                        if let Val::U8(x) = it {
                            b.push(*x);
                        }
                    }
                    b
                }
                other => {
                    return Err(Error::value(format!(
                        "expected list<u8>, got {}",
                        other.type_name()
                    )))
                }
            };
            let ctx2 = wasi_ctx(ctx.caller)?;
            match stream {
                2 => ctx2
                    .stdout
                    .write_all(&bytes)
                    .map_err(|e| Error::trap(e.to_string()))?,
                3 => ctx2
                    .stderr
                    .write_all(&bytes)
                    .map_err(|e| Error::trap(e.to_string()))?,
                _ => {
                    return Err(Error::trap(
                        "output-stream: only stdout/stderr are available",
                    ))
                }
            }
            out.push(Val::Result(Ok(None)));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "[method]input-stream.subscribe",
        |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            out.push(Val::Handle(Handle::Own {
                resource: 0,
                index: 0,
            }));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "[method]output-stream.subscribe",
        |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            out.push(Val::Handle(Handle::Own {
                resource: 0,
                index: 0,
            }));
            Ok(())
        },
    );

    // --- wasi:cli/environment ----------------------------------------------
    let key = format!("wasi:cli/environment@{v}");
    linker.func(
        Some(&key),
        "get-environment",
        |ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            let env = wasi_ctx(ctx.caller)?.env.clone();
            let pairs: Vec<Val> = env
                .into_iter()
                .map(|(k, v)| Val::Tuple(vec![Val::String(k), Val::String(v)]))
                .collect();
            out.push(Val::List(pairs));
            Ok(())
        },
    );
    linker.func(
        Some(&key),
        "initial-cwd",
        |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            out.push(Val::Option(None));
            Ok(())
        },
    );

    // --- wasi:cli/arguments -------------------------------------------------
    let key = format!("wasi:cli/arguments@{v}");
    linker.func(
        Some(&key),
        "get-arguments",
        |ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
            let args = wasi_ctx(ctx.caller)?.args.clone();
            out.push(Val::List(args.into_iter().map(Val::String).collect()));
            Ok(())
        },
    );

    // --- wasi:cli/exit ------------------------------------------------------
    let key = format!("wasi:cli/exit@{v}");
    linker.func(
        Some(&key),
        "exit",
        |ctx: &mut HostCtx<'_>, args: &[Val], _out: &mut Vec<Val>| {
            let code = match args.first() {
                Some(Val::Result(Ok(_))) => 0,
                Some(Val::Result(Err(Some(payload)))) => match payload.as_ref() {
                    Val::U8(c) => *c as i32,
                    Val::U32(c) => *c as i32,
                    _ => 1,
                },
                _ => 1,
            };
            wasi_ctx(ctx.caller)?.exit_code = Some(code);
            Err(Error::trap(format!("WASI exit({code})")))
        },
    );

    // --- wasi:cli/stdin, stdout, stderr --------------------------------------
    for (iface_suffix, name, rep) in [
        ("stdin", "get-stdin", 1u32),
        ("stdout", "get-stdout", 2u32),
        ("stderr", "get-stderr", 3u32),
    ] {
        let key = format!("wasi:cli/{iface_suffix}@{v}");
        linker.func(
            Some(&key),
            name,
            move |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
                // own<input-stream/output-stream>: host returns the rep (1..3);
                // the shim wraps it via resource-new so guest sees a handle
                out.push(Val::Handle(Handle::Own {
                    resource: 0,
                    index: rep,
                }));
                Ok(())
            },
        );
    }

    // --- wasi:cli/terminal-* -------------------------------------------------
    for iface in [
        "terminal-input",
        "terminal-output",
        "terminal-stdin",
        "terminal-stdout",
        "terminal-stderr",
    ] {
        let key = format!("wasi:cli/{iface}@{v}");
        linker.func(
            Some(&key),
            "get-terminal-stdin",
            |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
                out.push(Val::Option(None));
                Ok(())
            },
        );
        linker.func(
            Some(&key),
            "get-terminal-stdout",
            |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
                out.push(Val::Option(None));
                Ok(())
            },
        );
        linker.func(
            Some(&key),
            "get-terminal-stderr",
            |_ctx: &mut HostCtx<'_>, _args: &[Val], out: &mut Vec<Val>| {
                out.push(Val::Option(None));
                Ok(())
            },
        );
    }
}

fn stream_id_from(ctx: &mut HostCtx<'_>, handle: &Val) -> Result<u64> {
    // stream handles created by a real resource shim carry the original rep
    // inside the instance's resource table: resolve through the table first
    // (keyed by the handle's resource type), then fall back to a raw rep.
    match handle {
        Val::Handle(h) => {
            if let Ok(rep) = ctx
                .tables
                .lock()
                .unwrap()
                .resource_rep(h.resource(), h.index())
            {
                return Ok(std_stream_id(rep));
            }
            if let Ok(rep) = ctx.tables.lock().unwrap().resource_rep(u32::MAX, h.index()) {
                return Ok(std_stream_id(rep));
            }
            Ok(std_stream_id(h.index() as u64))
        }
        other => Err(Error::value(format!(
            "expected stream handle, found {}",
            other.type_name()
        ))),
    }
}

/// Convenience: register BOTH the P1 raw imports and the component interfaces.
pub fn add_all_to_linker(linker: &mut Linker, version: &str) {
    add_p1_to_linker(linker);
    add_to_linker(linker, version);
}

/// Shared-tables helper for embedders managing resources manually.
pub fn tables_of(instance: &crate::ComponentInstance) -> SharedTables {
    instance.tables().clone()
}

// ---------------------------------------------------------------------------
// One-call WASI runner
// ---------------------------------------------------------------------------

/// Run a WASI program end-to-end in a single call.
///
/// `wasm` may be a **core module** (WASI P1 guest, entry point `_start`,
/// or `_initialize` for reactors) or a **component binary** (WASI P2/P3
/// guest, entry point the `run` export of `wasi:cli/run`). The WASI host
/// calls are registered for both layers, so no additional setup is needed:
///
/// ```no_run
/// use waclay::{Engine, wasi::{self, WasiCtx}};
/// use waclay_wasmi::WasmiEngine;
///
/// # fn demo(wasm: &[u8]) -> waclay::Result<()> {
/// let engine = Engine::new(WasmiEngine::default());
/// let code = wasi::run(&engine, wasm, WasiCtx::builder().arg("prog").build())?;
/// assert_eq!(code, 0);
/// # Ok(())
/// # }
/// ```
///
/// A guest `proc_exit(n)` (P1) or `cli/exit(n)` (P2/P3) is translated into
/// `Ok(n)`; any other trap is returned as an [`Error`]. The
/// captured exit code is also recorded in the [`WasiCtx`] afterwards.
pub fn run(engine: &crate::Engine, wasm: &[u8], ctx: WasiCtx) -> Result<i32> {
    run_with(engine, wasm, ctx, |_| {})
}

/// Like [`run`], but lets the embedder add extra host functions to the
/// linker before instantiation (custom interfaces, mocks, ...).
pub fn run_with(
    engine: &crate::Engine,
    wasm: &[u8],
    ctx: WasiCtx,
    configure: impl FnOnce(&mut crate::Linker),
) -> Result<i32> {
    let is_component = wasm.len() >= 8 && wasm[4] == 0x0d;
    let component = if is_component {
        crate::Component::from_binary(engine, wasm)?
    } else {
        crate::Component::from_core_module(engine, wasm)?
    };

    let mut linker = crate::Linker::new();
    add_p1_to_linker(&mut linker);
    add_to_linker(&mut linker, "0.2.0");
    add_to_linker(&mut linker, "0.3.0");
    configure(&mut linker);

    let mut store = crate::Store::new(engine, ctx)?;
    let instance = linker.instantiate(&mut store, &component)?;

    let result = if instance.is_core_only() {
        if instance.get_core_export(&mut store, "_start").is_some() {
            instance.call_core(&mut store, "_start", &[], &mut [])
        } else if instance
            .get_core_export(&mut store, "_initialize")
            .is_some()
        {
            instance.call_core(&mut store, "_initialize", &[], &mut [])
        } else {
            return Err(Error::unsupported(
                "core module exports neither `_start` nor `_initialize`",
            ));
        }
    } else {
        instance.call_func(&mut store, "run", &[]).map(|_| ())
    };

    match result {
        Ok(()) => {}
        Err(err) => {
            // P1 `proc_exit` / P2+P3 `cli/exit` record the code in the ctx
            // *before* trapping; message parsing is only the fallback
            // (engines format nested trap messages differently).
            let code = ctx_exit_code(&mut store).or_else(|| exit_code_of(&err));
            if let Some(code) = code {
                set_exit_code(&mut store, code);
                return Ok(code);
            }
            return Err(err);
        }
    }

    Ok(store
        .data()
        .downcast_ref::<WasiCtx>()
        .and_then(|c| c.exit_code)
        .unwrap_or(0))
}

fn exit_code_of(err: &Error) -> Option<i32> {
    let msg = err.to_string();
    for tag in ["proc_exit(", "WASI exit("] {
        let Some(i) = msg.find(tag) else {
            continue;
        };
        let rest = &msg[i + tag.len()..];
        let mut digits = String::new();
        for c in rest.chars() {
            if c.is_ascii_digit() || (c == '-' && digits.is_empty()) {
                digits.push(c);
            } else if !digits.is_empty() {
                break;
            }
        }
        if let Ok(code) = digits.parse::<i32>() {
            return Some(code);
        }
    }
    None
}

fn set_exit_code(store: &mut crate::Store, code: i32) {
    if let Some(ctx) = store.data_mut().downcast_mut::<WasiCtx>() {
        ctx.exit_code = Some(code);
    }
}

fn ctx_exit_code(store: &mut crate::Store) -> Option<i32> {
    store
        .data()
        .downcast_ref::<WasiCtx>()
        .and_then(|c| c.exit_code)
}

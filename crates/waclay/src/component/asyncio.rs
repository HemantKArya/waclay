//! Component-model async runtime (preview 3): streams, futures, waitable
//! sets, error contexts and the task model.
//!
//! This module implements the *state machines* defined by the Component
//! Model spec (CanonicalABI.md):
//!
//! * `stream<T>` / `future<T>` ends ([`AsyncState::stream_new`], …)
//! * waitable sets ([`AsyncState::set_new`], `wait` / `poll` / `join`)
//! * error contexts ([`AsyncState::errctx_new`], …)
//! * backpressure counters, task-local storage and the `task.return` cell
//!
//! Everything is engine-agnostic: values move in and out as
//! [`Val`] vectors; lifting/lowering to linear memory is done by
//! the caller (the canon builtins in [`crate::linker`] and the host-facing
//! wrappers on [`crate::ComponentInstance`]).
//!
//! Semantics notes (synchronous embedder):
//!
//! * `stream`/`future` **writes** always complete immediately (the buffer is
//!   unbounded), matching the spec's "copy makes progress" rule.
//! * A **read** completes immediately when data is available or the other
//!   end was dropped; otherwise a *sync* read traps with a clear
//!   "would block" diagnostic (a synchronous embedder has no other tasks to
//!   run, so the spec's blocking wait could never wake up) and an *async*
//!   read returns `BLOCKED` and registers a pending copy that is completed
//!   by [`AsyncState::confirm_read`] once the host feeds the stream.
//! * Event codes / copy results / packed payloads follow the spec
//!   (`result | (progress << 4)`, `BLOCKED = 0xffff_ffff`).

use std::collections::VecDeque;

use crate::abi::StringEncoding;
use crate::component::types::{TypeRef, Val};
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Spec constants
// ---------------------------------------------------------------------------

/// `BLOCKED` sentinel returned by async operations that made no progress.
pub const BLOCKED: i32 = -1;

/// `EventCode` values delivered through waitable sets (CanonicalABI.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum EventCode {
    None = 0,
    Subtask = 1,
    StreamRead = 2,
    StreamWrite = 3,
    FutureRead = 4,
    FutureWrite = 5,
    TaskCancelled = 6,
}

impl EventCode {
    /// Numeric encoding used in core memory.
    pub fn raw(self) -> u32 {
        self as u32
    }

    /// Decode from the numeric encoding (spec-trapping on unknown codes).
    pub fn from_raw(v: u32) -> Result<EventCode> {
        Ok(match v {
            0 => EventCode::None,
            1 => EventCode::Subtask,
            2 => EventCode::StreamRead,
            3 => EventCode::StreamWrite,
            4 => EventCode::FutureRead,
            5 => EventCode::FutureWrite,
            6 => EventCode::TaskCancelled,
            _ => return Err(Error::value(format!("invalid event code {v}"))),
        })
    }
}

/// `CopyResult` codes packed into stream/future event payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CopyResult {
    Completed = 0,
    Dropped = 1,
    Cancelled = 2,
}

/// `Subtask.State` codes (CanonicalABI.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SubtaskState {
    Starting = 0,
    Started = 1,
    Returned = 2,
    CancelledBeforeStarted = 3,
    CancelledBeforeReturned = 4,
}

/// `CallbackCode` packing of async-lifted entry/callback results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CallbackCode {
    Exit = 0,
    Yield = 1,
    Wait = 2,
}

/// Unpack an async-lifted entry/callback result: low 4 bits = code,
/// remaining bits = waitable-set index.
pub fn unpack_callback_result(packed: i32) -> Result<(CallbackCode, u32)> {
    let packed = packed as u32;
    let code = packed & 0xf;
    let si = packed >> 4;
    Ok(match code {
        0 => (CallbackCode::Exit, si),
        1 => (CallbackCode::Yield, si),
        2 => (CallbackCode::Wait, si),
        _ => return Err(Error::value(format!("invalid callback code {code}"))),
    })
}

// ---------------------------------------------------------------------------
// 1-based handle tables (index 0 is the invalid sentinel, per spec tables)
// ---------------------------------------------------------------------------

struct Table<T> {
    slots: Vec<Option<T>>,
    free: Vec<u32>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Table {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<T> Table<T> {
    fn alloc(&mut self, v: T) -> u32 {
        if let Some(idx) = self.free.pop() {
            self.slots[idx as usize] = Some(v);
            idx + 1
        } else {
            self.slots.push(Some(v));
            self.slots.len() as u32
        }
    }

    fn get(&self, h: u32) -> Option<&T> {
        if h == 0 {
            return None;
        }
        self.slots.get((h - 1) as usize).and_then(|s| s.as_ref())
    }

    fn get_mut(&mut self, h: u32) -> Option<&mut T> {
        if h == 0 {
            return None;
        }
        self.slots
            .get_mut((h - 1) as usize)
            .and_then(|s| s.as_mut())
    }

    fn remove(&mut self, h: u32) -> Option<T> {
        if h == 0 {
            return None;
        }
        let slot = self.slots.get_mut((h - 1) as usize).and_then(|s| s.take());
        if slot.is_some() {
            self.free.push(h - 1);
        }
        slot
    }
}

// ---------------------------------------------------------------------------
// Stream / future ends
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndKind {
    StreamReadable,
    StreamWritable,
    FutureReadable,
    FutureWritable,
}

impl EndKind {
    fn event_code(self) -> EventCode {
        match self {
            EndKind::StreamReadable => EventCode::StreamRead,
            EndKind::StreamWritable => EventCode::StreamWrite,
            EndKind::FutureReadable => EventCode::FutureRead,
            EndKind::FutureWritable => EventCode::FutureWrite,
        }
    }

    #[allow(dead_code)]
    fn is_readable(self) -> bool {
        matches!(self, EndKind::StreamReadable | EndKind::FutureReadable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndState {
    /// No operation in progress; read/write may start.
    Idle,
    /// A pending (async) copy is registered.
    Copying,
    /// Terminal: only `drop-*` remains valid.
    Done,
}

/// Bookkeeping for a pending async read (guest buffer that will receive
/// elements once data arrives).
#[derive(Debug, Clone)]
pub struct PendingRead {
    /// Guest buffer pointer.
    pub ptr: u32,
    /// Requested element count.
    pub count: u32,
    /// Element type (for storing values into guest memory).
    pub elem: Option<TypeRef>,
    /// String encoding for element storing.
    pub enc: StringEncoding,
    /// The component type table (needed to size/store elements).
    pub types: std::sync::Arc<Vec<crate::component::types::TypeDef>>,
}

struct End {
    ty: u32,
    kind: EndKind,
    state: EndState,
    /// Handle of the opposite end (`0` once the other side is dropped).
    other: u32,
    /// Index into [`AsyncState::pipes`] / [`AsyncState::cells`] shared by the
    /// two ends of one stream/future.
    pair: usize,
    pending: Option<(EventCode, u32, u32)>,
    wset: Option<u32>,
    /// Pending async read registered by `stream.read`/`future.read`.
    reading: Option<PendingRead>,
}

#[derive(Default)]
struct StreamPipe {
    buf: VecDeque<Val>,
    writer_gone: bool,
    reader_gone: bool,
}

#[derive(Default)]
struct FutureCell {
    value: Option<Val>,
    /// A value was written at least once (sticky: survives `future.read`
    /// consuming the value, so `future.drop-writable` no longer traps).
    written: bool,
    writer_gone: bool,
    reader_gone: bool,
}

#[derive(Default)]
struct WaitableSet {
    /// Member end handles in join order.
    members: Vec<u32>,
}

fn trap(msg: impl Into<String>) -> Error {
    Error::trap(msg.into())
}

/// Per-instance async state: waitable tables, waitable sets, error contexts,
/// backpressure and task-local storage. Shared between the canon builtins
/// (guest side) and the host-facing API via `Arc<Mutex<..>>`.
#[derive(Default)]
pub struct AsyncState {
    ends: Table<End>,
    pipes: Vec<StreamPipe>,
    cells: Vec<FutureCell>,
    sets: Table<WaitableSet>,
    errctxs: Table<String>,
    backpressure: u32,
    /// Task-local storage slots (`context.get` / `context.set`).
    pub context_slots: [u64; 2],
    /// Value delivered by `task.return` for the current async-lifted call.
    pub task_result: Option<Vec<Val>>,
}

impl AsyncState {
    // -- construction -------------------------------------------------------

    /// Create a new stream pair: returns `(readable, writable)` handles.
    pub fn stream_new(&mut self, ty: u32) -> (u32, u32) {
        let pair = self.pipes.len();
        self.pipes.push(StreamPipe::default());
        self.make_pair(ty, pair, true)
    }

    /// Create a new future pair: returns `(readable, writable)` handles.
    pub fn future_new(&mut self, ty: u32) -> (u32, u32) {
        let pair = self.cells.len();
        self.cells.push(FutureCell::default());
        self.make_pair(ty, pair, false)
    }

    fn make_pair(&mut self, ty: u32, pair: usize, is_stream: bool) -> (u32, u32) {
        let (rk, wk) = if is_stream {
            (EndKind::StreamReadable, EndKind::StreamWritable)
        } else {
            (EndKind::FutureReadable, EndKind::FutureWritable)
        };
        let r = self.ends.alloc(End {
            ty,
            kind: rk,
            state: EndState::Idle,
            other: 0,
            pair,
            pending: None,
            wset: None,
            reading: None,
        });
        let w = self.ends.alloc(End {
            ty,
            kind: wk,
            state: EndState::Idle,
            other: 0,
            pair,
            pending: None,
            wset: None,
            reading: None,
        });
        self.ends.get_mut(r).unwrap().other = w;
        self.ends.get_mut(w).unwrap().other = r;
        (r, w)
    }

    fn end(&self, ty: u32, h: u32, want: EndKind) -> Result<&End> {
        let e = self
            .ends
            .get(h)
            .ok_or_else(|| trap(format!("invalid stream/future handle {h}")))?;
        if e.kind != want {
            return Err(trap(format!(
                "handle {h} is the wrong end (got {:?}, want {:?})",
                e.kind, want
            )));
        }
        if e.ty != ty {
            return Err(trap(format!(
                "handle {h} belongs to stream/future type #{}, expected #{ty}",
                e.ty
            )));
        }
        Ok(e)
    }

    // -- spec event computation --------------------------------------------

    /// Compute and install the pending event for a stream end, per the
    /// spec's `stream_event` closure.
    fn set_stream_event(&mut self, h: u32, progress: u32) -> (EventCode, u32, u32) {
        let e = self.ends.get_mut(h).unwrap();
        let result = if e.other == 0 {
            e.state = EndState::Done;
            CopyResult::Dropped
        } else {
            e.state = EndState::Idle;
            CopyResult::Completed
        };
        let packed = (result as u32) | (progress << 4);
        let code = e.kind.event_code();
        e.pending = Some((code, h, packed));
        (code, h, packed)
    }

    /// Compute and install the pending event for a future end, per the
    /// spec's `future_event` closure.
    fn set_future_event(&mut self, h: u32, progress: u32) -> (EventCode, u32, u32) {
        let e = self.ends.get_mut(h).unwrap();
        let result = if progress == 1 {
            e.state = EndState::Done;
            CopyResult::Completed
        } else if e.other == 0 {
            e.state = EndState::Done;
            CopyResult::Dropped
        } else {
            e.state = EndState::Idle;
            CopyResult::Cancelled
        };
        let code = e.kind.event_code();
        e.pending = Some((code, h, result as u32));
        (code, h, result as u32)
    }

    fn consume_event(&mut self, h: u32) {
        if let Some(e) = self.ends.get_mut(h) {
            e.pending.take();
        }
    }

    // -- builtin ops (guest side) -------------------------------------------

    /// `stream.read`: pull up to `max` buffered values.
    ///
    /// Returns the packed payload for the guest plus the values to store
    /// into guest memory (empty for `BLOCKED`).
    pub fn stream_read(
        &mut self,
        ty: u32,
        h: u32,
        max: u32,
        async_: bool,
    ) -> Result<(i32, Vec<Val>)> {
        self.end(ty, h, EndKind::StreamReadable)?;
        {
            let e = self.ends.get(h).unwrap();
            if e.state == EndState::Done {
                return Err(trap("stream.read on a done (dropped) stream end"));
            }
            if e.state == EndState::Copying {
                return Err(trap("stream.read: a read is already in progress"));
            }
        }
        let pair = self.ends.get(h).unwrap().pair;
        let available = self.pipes[pair].buf.len().min(max as usize);
        let writer_gone = self.pipes[pair].writer_gone;
        if available > 0 {
            let vals: Vec<Val> = self.pipes[pair].buf.drain(..available).collect();
            if writer_gone && self.pipes[pair].buf.is_empty() {
                // data transferred, writer gone: the *next* read is DROPPED;
                // this one still completes normally
                let packed = (CopyResult::Completed as u32) | ((available as u32) << 4);
                return Ok((packed as i32, vals));
            }
            let packed = (CopyResult::Completed as u32) | ((available as u32) << 4);
            return Ok((packed as i32, vals));
        }
        if writer_gone {
            let (_, _, packed) = self.set_stream_event(h, 0);
            let done = self.ends.get(h).unwrap().state == EndState::Done;
            self.consume_event(h);
            debug_assert!(done || packed == CopyResult::Dropped as u32);
            return Ok((packed as i32, Vec::new()));
        }
        if async_ {
            let e = self.ends.get_mut(h).unwrap();
            e.state = EndState::Copying;
            return Ok((BLOCKED, Vec::new()));
        }
        Err(trap(
            "stream.read would block: the buffer is empty and the write end is \
             still open; feed data first (host stream API) or use the `async` \
             canon option",
        ))
    }

    /// `stream.write`: push values into the pipe.
    ///
    /// Returns the packed payload plus, when the opposite end had a pending
    /// async read, the pending descriptor and the values that must be stored
    /// into guest memory by the caller.
    pub fn stream_write(
        &mut self,
        ty: u32,
        h: u32,
        vals: Vec<Val>,
    ) -> Result<(i32, Option<PendingRead>, Vec<Val>)> {
        self.end(ty, h, EndKind::StreamWritable)?;
        {
            let e = self.ends.get(h).unwrap();
            if e.state == EndState::Done {
                return Err(trap("stream.write on a done (dropped) stream end"));
            }
        }
        let other = self.ends.get(h).unwrap().other;
        let count = vals.len() as u32;
        let pending = if other != 0 {
            self.ends.get_mut(other).unwrap().reading.take()
        } else {
            None
        };
        if let Some(p) = pending {
            let take = (p.count as usize).min(vals.len());
            let mut iter = vals.into_iter();
            let drained: Vec<Val> = iter.by_ref().take(take).collect();
            let leftover: Vec<Val> = iter.collect();
            if !leftover.is_empty() {
                let pair = self.ends.get(h).unwrap().pair;
                self.pipes[pair].buf.extend(leftover);
            }
            let (_, _, packed) = self.set_stream_event(other, drained.len() as u32);
            self.consume_event(other);
            return Ok((packed as i32, Some(p), drained));
        }
        let pair = self.ends.get(h).unwrap().pair;
        self.pipes[pair].buf.extend(vals);
        let packed = (CopyResult::Completed as u32) | (count << 4);
        Ok((packed as i32, None, Vec::new()))
    }

    /// `future.read`.
    pub fn future_read(&mut self, ty: u32, h: u32, async_: bool) -> Result<(i32, Option<Val>)> {
        self.end(ty, h, EndKind::FutureReadable)?;
        {
            let e = self.ends.get(h).unwrap();
            if e.state == EndState::Done {
                return Err(trap("future.read on a done (resolved/dropped) future end"));
            }
            if e.state == EndState::Copying {
                return Err(trap("future.read: a read is already in progress"));
            }
        }
        let pair = self.ends.get(h).unwrap().pair;
        if let Some(v) = self.cells[pair].value.take() {
            let (_, _, packed) = self.set_future_event(h, 1);
            self.consume_event(h);
            return Ok((packed as i32, Some(v)));
        }
        if self.cells[pair].writer_gone {
            let (_, _, packed) = self.set_future_event(h, 0);
            self.consume_event(h);
            return Ok((packed as i32, None));
        }
        if async_ {
            let e = self.ends.get_mut(h).unwrap();
            e.state = EndState::Copying;
            return Ok((BLOCKED, None));
        }
        Err(trap(
            "future.read would block: no value has been written yet and the \
             write end is still open; use the `async` canon option",
        ))
    }

    /// `future.write`.
    pub fn future_write(
        &mut self,
        ty: u32,
        h: u32,
        val: Val,
    ) -> Result<(i32, Option<PendingRead>, Option<Val>)> {
        self.end(ty, h, EndKind::FutureWritable)?;
        {
            let e = self.ends.get(h).unwrap();
            if e.state == EndState::Done {
                return Err(trap("future.write on a done (resolved/dropped) future end"));
            }
        }
        let other = self.ends.get(h).unwrap().other;
        // writing into a future whose readable end was dropped makes no
        // progress: the event is DROPPED and the value is discarded
        if other == 0 {
            let (_, _, packed) = self.set_future_event(h, 0);
            self.consume_event(h);
            return Ok((packed as i32, None, None));
        }
        let pending = self.ends.get_mut(other).unwrap().reading.take();
        if let Some(p) = pending {
            // deliver straight to the waiting async reader
            let (_, _, packed) = self.set_future_event(other, 1);
            self.consume_event(other);
            return Ok((packed as i32, Some(p), Some(val)));
        }
        let pair = self.ends.get(h).unwrap().pair;
        self.cells[pair].value = Some(val);
        self.cells[pair].written = true;
        let (_, _, packed) = self.set_future_event(h, 1);
        self.consume_event(h);
        Ok((packed as i32, None, None))
    }

    /// Register the guest buffer of an async read that returned `BLOCKED`
    /// (called by the canon builtin right after `stream_read`/`future_read`).
    pub fn register_read(&mut self, h: u32, p: PendingRead) -> Result<()> {
        let e = self
            .ends
            .get_mut(h)
            .ok_or_else(|| trap(format!("invalid stream/future handle {h}")))?;
        if e.state != EndState::Copying {
            return Err(trap("register_read: no async read is in progress"));
        }
        e.reading = Some(p);
        Ok(())
    }

    /// `{stream,future}.cancel-{read,write}`: cancel a pending async copy.
    /// Returns the `CANCELLED` payload.
    pub fn cancel_copy(
        &mut self,
        ty: u32,
        h: u32,
        want_readable: bool,
        is_stream: bool,
    ) -> Result<i32> {
        let want = kinds(want_readable, is_stream).0;
        self.end(ty, h, want)?;
        let e = self.ends.get_mut(h).unwrap();
        if e.state != EndState::Copying {
            return Err(trap(
                "cancel-read/cancel-write requires an in-progress async copy",
            ));
        }
        e.state = EndState::Idle;
        e.reading = None;
        Ok(CopyResult::Cancelled as i32)
    }

    /// `{stream,future}.drop-{readable,writable}`.
    pub fn drop_end(
        &mut self,
        ty: u32,
        h: u32,
        want_readable: bool,
        is_stream: bool,
    ) -> Result<()> {
        let want = kinds(want_readable, is_stream).0;
        self.end(ty, h, want)?;
        {
            let e = self.ends.get(h).unwrap();
            if e.state == EndState::Copying {
                return Err(trap(
                    "cannot drop a stream/future end with a copy in progress",
                ));
            }
            // future.drop-writable traps when no value was ever written and
            // the readable end was already dropped
            if !is_stream && !want_readable {
                let pair = e.pair;
                if !self.cells[pair].written && self.cells[pair].reader_gone {
                    return Err(trap(
                        "future.drop-writable: no value was written and the \
                         readable end was already dropped",
                    ));
                }
            }
        }
        let (other, pair, kind) = {
            let e = self.ends.get(h).unwrap();
            (e.other, e.pair, e.kind)
        };
        match kind {
            EndKind::StreamReadable => self.pipes[pair].reader_gone = true,
            EndKind::StreamWritable => self.pipes[pair].writer_gone = true,
            EndKind::FutureReadable => self.cells[pair].reader_gone = true,
            EndKind::FutureWritable => self.cells[pair].writer_gone = true,
        }
        // detach from its waitable set
        if let Some(e) = self.ends.get_mut(h) {
            if let Some(si) = e.wset.take() {
                if let Some(s) = self.sets.get_mut(si) {
                    s.members.retain(|m| *m != h);
                }
            }
        }
        // sever the peer first so the notification reports DROPPED
        if other != 0 {
            if let Some(o) = self.ends.get_mut(other) {
                o.other = 0;
            }
        }
        self.ends.remove(h);
        // wake the peer if it has a pending async read
        if other != 0 {
            let copying = self
                .ends
                .get(other)
                .map(|o| o.state == EndState::Copying)
                .unwrap_or(false);
            if copying {
                let okind = self.ends.get(other).unwrap().kind;
                let _ = if matches!(okind, EndKind::StreamReadable) {
                    self.set_stream_event(other, 0)
                } else {
                    self.set_future_event(other, 0)
                };
            }
        }
        Ok(())
    }

    /// `{stream,future}.forward`: splice the readable end `ri` into the
    /// writable end `wi`, transferring both out of the guest's hands.
    pub fn forward(&mut self, ty: u32, ri: u32, wi: u32, is_stream: bool) -> Result<()> {
        let (rk, wk) = kinds(true, is_stream);
        let _ = wk;
        self.end(ty, ri, rk)?;
        self.end(ty, wi, kinds(false, is_stream).0)?;
        if is_stream {
            let src = self.ends.get(ri).unwrap().pair;
            let dst = self.ends.get(wi).unwrap().pair;
            let moved: VecDeque<Val> = std::mem::take(&mut self.pipes[src].buf);
            self.pipes[dst].buf.extend(moved);
        } else {
            let src = self.ends.get(ri).unwrap().pair;
            let dst = self.ends.get(wi).unwrap().pair;
            if let Some(v) = self.cells[src].value.take() {
                if self.cells[dst].value.is_none() {
                    self.cells[dst].value = Some(v);
                }
            }
        }
        let r_other = self.ends.get(ri).unwrap().other;
        let w_other = self.ends.get(wi).unwrap().other;
        // detach membership
        for h in [ri, wi] {
            if let Some(e) = self.ends.get_mut(h) {
                if let Some(si) = e.wset.take() {
                    if let Some(s) = self.sets.get_mut(si) {
                        s.members.retain(|m| *m != h);
                    }
                }
            }
        }
        self.ends.remove(ri);
        self.ends.remove(wi);
        if r_other != 0 {
            if let Some(o) = self.ends.get_mut(r_other) {
                o.other = w_other;
            }
        }
        if w_other != 0 {
            if let Some(o) = self.ends.get_mut(w_other) {
                o.other = r_other;
            }
        }
        Ok(())
    }

    // -- waitable sets -------------------------------------------------------

    /// `waitable-set.new`.
    pub fn set_new(&mut self) -> u32 {
        self.sets.alloc(WaitableSet::default())
    }

    /// `waitable.join`: add a stream/future end to a set (`si == 0` detaches).
    pub fn join(&mut self, wi: u32, si: u32) -> Result<()> {
        if self.ends.get(wi).is_none() {
            return Err(trap(format!("waitable.join: invalid waitable handle {wi}")));
        }
        if si == 0 {
            if let Some(e) = self.ends.get_mut(wi) {
                if let Some(old) = e.wset.take() {
                    if let Some(s) = self.sets.get_mut(old) {
                        s.members.retain(|m| *m != wi);
                    }
                }
            }
            return Ok(());
        }
        if self.sets.get(si).is_none() {
            return Err(trap(format!(
                "waitable.join: invalid waitable-set handle {si}"
            )));
        }
        if let Some(e) = self.ends.get_mut(wi) {
            if let Some(old) = e.wset.take() {
                if let Some(s) = self.sets.get_mut(old) {
                    s.members.retain(|m| *m != wi);
                }
            }
            e.wset = Some(si);
        }
        let s = self.sets.get_mut(si).unwrap();
        if !s.members.contains(&wi) {
            s.members.push(wi);
        }
        Ok(())
    }

    /// Deliver the pending event of an end.
    fn deliver(&mut self, h: u32) -> Option<(EventCode, u32, u32)> {
        self.ends.get_mut(h)?.pending.take()
    }

    /// `waitable-set.wait`: deliver the first ready event (traps when none).
    pub fn set_wait(&mut self, si: u32) -> Result<(EventCode, u32, u32)> {
        let members = self
            .sets
            .get(si)
            .ok_or_else(|| trap(format!("invalid waitable-set handle {si}")))?
            .members
            .clone();
        for m in members {
            if self
                .ends
                .get(m)
                .map(|e| e.pending.is_some())
                .unwrap_or(false)
            {
                if let Some(ev) = self.deliver(m) {
                    return Ok(ev);
                }
            }
        }
        Err(trap(
            "waitable-set.wait would block: no events are ready; a synchronous \
             host cannot wake this wait — use waitable-set.poll or ensure \
             progress before waiting",
        ))
    }

    /// `waitable-set.poll`: deliver the first ready event or `None`.
    pub fn set_poll(&mut self, si: u32) -> Result<Option<(EventCode, u32, u32)>> {
        let members = self
            .sets
            .get(si)
            .ok_or_else(|| trap(format!("invalid waitable-set handle {si}")))?
            .members
            .clone();
        for m in members {
            if self
                .ends
                .get(m)
                .map(|e| e.pending.is_some())
                .unwrap_or(false)
            {
                if let Some(ev) = self.deliver(m) {
                    return Ok(Some(ev));
                }
            }
        }
        Ok(None)
    }

    /// `waitable-set.drop` (traps when the set still has members).
    pub fn set_drop(&mut self, si: u32) -> Result<()> {
        let empty = self
            .sets
            .get(si)
            .ok_or_else(|| trap(format!("invalid waitable-set handle {si}")))?
            .members
            .is_empty();
        if !empty {
            return Err(trap("waitable-set.drop: the set still has members"));
        }
        self.sets.remove(si);
        Ok(())
    }

    // -- error contexts -------------------------------------------------------

    /// `error-context.new`.
    pub fn errctx_new(&mut self, msg: String) -> u32 {
        self.errctxs.alloc(msg)
    }

    /// `error-context.debug-message`.
    pub fn errctx_debug_message(&self, h: u32) -> Result<String> {
        self.errctxs
            .get(h)
            .cloned()
            .ok_or_else(|| trap(format!("invalid error-context handle {h}")))
    }

    /// `error-context.drop`.
    pub fn errctx_drop(&mut self, h: u32) -> Result<()> {
        self.errctxs
            .remove(h)
            .ok_or_else(|| trap(format!("invalid error-context handle {h}")))?;
        Ok(())
    }

    // -- backpressure / subtasks ----------------------------------------------

    /// `backpressure.inc`.
    pub fn backpressure_inc(&mut self) -> Result<()> {
        self.backpressure = self
            .backpressure
            .checked_add(1)
            .filter(|v| *v < 1 << 16)
            .ok_or_else(|| trap("backpressure counter overflow"))?;
        Ok(())
    }

    /// `backpressure.dec`.
    pub fn backpressure_dec(&mut self) -> Result<()> {
        self.backpressure = self
            .backpressure
            .checked_sub(1)
            .ok_or_else(|| trap("backpressure counter underflow"))?;
        Ok(())
    }

    /// Current backpressure count (diagnostics).
    pub fn backpressure(&self) -> u32 {
        self.backpressure
    }

    /// `subtask.drop` — no unresolved subtasks exist in a synchronous
    /// runtime, so any handle is invalid (spec-trapping is correct).
    pub fn subtask_drop(&mut self, _h: u32) -> Result<()> {
        Err(trap(
            "invalid subtask handle (subtasks do not occur in a synchronous runtime)",
        ))
    }

    /// `subtask.cancel`.
    pub fn subtask_cancel(&mut self, _h: u32, _async_: bool) -> Result<i32> {
        Err(trap(
            "invalid subtask handle (subtasks do not occur in a synchronous runtime)",
        ))
    }

    // -- host-facing API -------------------------------------------------------

    /// Host feed: push values into a stream (buffer) or future (cell).
    ///
    /// When the readable end has a pending async read, returns
    /// [`HostWrite::Deliver`] so the caller can store the values into guest
    /// memory and then call [`AsyncState::confirm_read`].
    pub fn host_write(
        &mut self,
        ty: u32,
        h: u32,
        vals: Vec<Val>,
        is_stream: bool,
    ) -> Result<HostWrite> {
        let want = if is_stream {
            EndKind::StreamWritable
        } else {
            EndKind::FutureWritable
        };
        self.end(ty, h, want)?;
        let (other, pair) = {
            let e = self.ends.get(h).unwrap();
            (e.other, e.pair)
        };
        let pending = if other != 0 {
            self.ends.get_mut(other).unwrap().reading.take()
        } else {
            None
        };
        if let Some(p) = pending {
            let take = (p.count as usize).min(vals.len());
            let mut iter = vals.into_iter();
            let drained: Vec<Val> = iter.by_ref().take(take).collect();
            let leftover: Vec<Val> = iter.collect();
            if !leftover.is_empty() && is_stream {
                self.pipes[pair].buf.extend(leftover);
            }
            return Ok(HostWrite::Deliver {
                pending: p,
                vals: drained,
                end: other,
            });
        }
        if is_stream {
            self.pipes[pair].buf.extend(vals);
        } else if self.cells[pair].value.is_none() {
            self.cells[pair].value = vals.into_iter().next();
            self.cells[pair].written = true;
        }
        Ok(HostWrite::Buffered)
    }

    /// Host drain: pull up to `max` buffered values from a readable end.
    /// Returns the values plus `true` when the writer is gone and nothing
    /// is buffered (end of stream).
    pub fn host_read(
        &mut self,
        ty: u32,
        h: u32,
        max: u32,
        is_stream: bool,
    ) -> Result<(Vec<Val>, bool)> {
        let want = if is_stream {
            EndKind::StreamReadable
        } else {
            EndKind::FutureReadable
        };
        self.end(ty, h, want)?;
        let pair = self.ends.get(h).unwrap().pair;
        if is_stream {
            let n = self.pipes[pair].buf.len().min(max as usize);
            let vals: Vec<Val> = self.pipes[pair].buf.drain(..n).collect();
            let done = self.pipes[pair].writer_gone && self.pipes[pair].buf.is_empty();
            Ok((vals, done))
        } else {
            let v = self.cells[pair].value.take();
            let done = self.cells[pair].writer_gone;
            Ok((v.into_iter().collect(), done))
        }
    }

    /// Complete a pending async read after the caller stored the values into
    /// guest memory (counterpart of [`AsyncState::host_write`]).
    pub fn confirm_read(&mut self, end: u32, count: u32, is_stream: bool) {
        if self
            .ends
            .get(end)
            .map(|e| e.state == EndState::Copying)
            .unwrap_or(false)
        {
            if is_stream {
                self.set_stream_event(end, count);
            } else {
                self.set_future_event(end, 1);
            }
        }
    }

    /// Diagnostics: number of live ends (host/debug).
    pub fn live_ends(&self) -> usize {
        (1..=self.ends.slots.len() as u32)
            .filter(|h| self.ends.get(*h).is_some())
            .count()
    }

    /// Diagnostics: number of live waitable sets.
    pub fn live_sets(&self) -> usize {
        (1..=self.sets.slots.len() as u32)
            .filter(|h| self.sets.get(*h).is_some())
            .count()
    }
}

fn kinds(readable: bool, is_stream: bool) -> (EndKind, EndKind) {
    match (readable, is_stream) {
        (true, true) => (EndKind::StreamReadable, EndKind::StreamWritable),
        (false, true) => (EndKind::StreamWritable, EndKind::StreamReadable),
        (true, false) => (EndKind::FutureReadable, EndKind::FutureWritable),
        (false, false) => (EndKind::FutureWritable, EndKind::FutureReadable),
    }
}

/// Result of a host-side write into a stream/future.
pub enum HostWrite {
    /// Values were buffered; no guest interaction pending.
    Buffered,
    /// A pending async guest read was found: store `vals` into guest memory
    /// per `pending`, then call [`AsyncState::confirm_read`] with `end`.
    Deliver {
        pending: PendingRead,
        vals: Vec<Val>,
        /// Handle of the readable end (for `confirm_read`).
        end: u32,
    },
}

/// Shared, lock-protected async state (host code + canon builtins).
pub type SharedAsync = std::sync::Arc<std::sync::Mutex<AsyncState>>;

/// Create a new shared async state.
pub fn shared() -> SharedAsync {
    std::sync::Arc::new(std::sync::Mutex::new(AsyncState::default()))
}

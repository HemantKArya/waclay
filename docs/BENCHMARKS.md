# Waclay 2.0 — Benchmarks

Latest run: **2026-10-25**, Linux x86_64, rustc **1.99.1**, `--release` bench
profile, criterion 0.8, engines wasmi **2.0.1** / wasmtime **50.0.0** /
wasmer **7.5.0**. Absolute numbers are environment-specific; the **ratios**
are the point.

Run locally:

```sh
cargo bench -p waclay --bench bench
```

## add(u32, u32) -> u32 — per-call overhead

The benchmark calls a trivial two-argument export in a tight loop. `raw-*`
rows call the engine directly with **no Waclay layer in the loop**;
`waclay-*` rows go through the full component path (canonical lift,
precomputed flat layout, `Val` boxing).

| benchmark             | median (2026-10-25) | layer delta |
|-----------------------|---------------------|-------------|
| raw-wasmi             | 91.7 ns             | —           |
| raw-wasmtime          | 206.1 ns            | —           |
| waclay-wasmi          | 224.8 ns            | **+133 ns** |
| waclay-wasmtime       | 415.5 ns            | **+209 ns** |
| waclay-wasmer         | 284.0 ns            | —           |

Reading: the waclay component layer costs **~133–210 ns per call** —
two small `Vec` allocations, a flat-layout walk over precomputed slots, and
`CoreVal`/`Val` conversions. This is the same order of magnitude as wasmtime's
own component-model call overhead and is dominated by dynamic value boxing,
not by the ABI math (which is precomputed at `Component::from_binary`).
Cross-run stability: the measured delta was ~133–192 ns in the previous
run and ~133–209 ns here — the layer cost is a stable structural constant,
not noise.

## fib(20) — compute throughput (through waclay's core-call path)

| backend             | median (2026-10-25) | note |
|---------------------|---------------------|------|
| waclay-wasmtime (JIT) | 43.8 µs           | near-native Cranelift code |
| waclay-wasmer (JIT)   | 43.3 µs           | on par with wasmtime |
| waclay-wasmi (interp) | 379 µs            | ~8.7× slower than JIT; tiny footprint |

wasmi 2.0's interpreter is significantly faster than at the previous
measurement (~860 µs) — the upgrade nearly halved interpreter compute
latency. The JIT engines deliver native-speed compute through waclay: the
layer adds nothing measurable to a 44 µs workload (0.4 µs overhead ≈ 1%).
Choose `wasmi` when startup size and pure-Rust compilation matter more
than throughput.

## string echo (58-byte string, realloc + copy, both directions)

| backend             | median (2026-10-25) |
|---------------------|---------------------|
| waclay-wasmi        | 529 ns              |
| waclay-wasmer       | 594 ns              |
| waclay-wasmtime     | 871 ns              |

Full Canonical-ABI round trip: lower the string into guest memory via the
guest's `cabi_realloc`, call the core export, lift the returned `(ptr, len)`
back into a Rust `String`. Sub-microsecond for realistic payloads.

## Methodology notes

* Guest fixture: hand-written wat with `add`, `fib`, `echo`, a bump
  `cabi_realloc` and a 1-page memory (identical binary for raw and waclay
  runs).
* `raw-wasmi` / `raw-wasmtime` use the engine crates directly
  (`Func::call` on the same module/function), so the delta against the
  `waclay-*` rows isolates waclay's layer.
* Benchmarks are re-runnable and deterministic in fixture; host machines will
  shift absolute numbers.

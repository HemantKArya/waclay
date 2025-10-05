# Bug Fix: Option<Record> Parameter Handling in ABI Generator

## Issue
When calling WASM component functions with `option<record>` parameters (e.g., `search(query: string, options: option<search-options>)`), the ABI generator was producing incorrect stack counts, causing an assertion failure:

```
assertion `left == right` failed
  left: 8
 right: 9
```

## Root Cause
In `waclay/src/abi.rs`, the `lower_variant_arm` function was unconditionally popping a value from the stack after calling `ExtractVariantDiscriminant`, even when the variant had no payload (e.g., `None` case of an `Option`).

The `ExtractVariantDiscriminant` instruction produces:
- 1 value (the payload) when the variant has a payload (indicated by `has_payload` = true)
- 0 values when the variant has no payload (indicated by `has_payload` = false)

The bug was that the code always called `self.stack.pop()` regardless of whether a payload was present, which would incorrectly pop values from earlier parameters (like the query string) when processing `None` values.

## Fix
Modified `lower_variant_arm` to conditionally pop based on the `has_payload` flag returned by `ExtractVariantDiscriminant`:

```rust
let (discriminant, has_payload) = if let ExtractVariantDiscriminant { discriminant_value } = disc_val {
    let (disc, has_payload) = discriminant_value.get();
    (disc, has_payload)
} else {
    unreachable!()
};

// ... 

let payload_name = if has_payload {
    self.stack.pop()  // Only pop if there's actually a payload
} else {
    None             // No payload, nothing to pop
};
```

## Testing
The fix was verified with the mediasource example, which has a function:
```wit
search: func(query: string, options: option<search-options>) -> search-response;
```

where `search-options` is:
```wit
record search-options {
    search-type: option<string>,
    page-token: option<string>,
}
```

Both test cases now pass:
- `search("query", None)` ✅
- `search("query", Some(SearchOptions { ... }))` ✅

## Impact
This fix resolves ABI generation issues for any component function that has:
- `option<record>` parameters
- `option<variant>` parameters
- `result<record, error>` parameters where either ok or err case is a record
- Any variant type with optional payloads containing complex types

The fix ensures proper stack management when lowering variant types with optional payloads.

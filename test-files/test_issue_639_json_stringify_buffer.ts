// Issue #639: JSON.stringify of a Buffer (or any value containing a Buffer)
// silently exited the process. BufferHeader has no GcHeader, so the JSON
// dispatch's `gc_obj_type(ptr)` (which reads 8 bytes before the header)
// read unrelated memory and routed to the wrong stringify arm — usually
// `is_object_pointer` deref'ing a bogus `keys_array` pointer and faulting.
//
// Surfaces in the wild as `@perryts/mysql`'s `pool.query()` result —
// `QueryResult.rowsRaw` is `RawRow[]` = `Buffer[][]`, so the result-as-a-whole
// stringify path always hit a Buffer field.

const buf = Buffer.from([1, 2, 3]);
console.log("buf alone:", JSON.stringify(buf));
console.log("[buf]:", JSON.stringify([buf]));
console.log("{b: buf}:", JSON.stringify({ b: buf }));

const u = new Uint8Array([4, 5, 6]);
console.log("u alone:", JSON.stringify(u));
console.log("[u]:", JSON.stringify([u]));
console.log("{u: u}:", JSON.stringify({ u: u }));

// Mixed: object containing arrays of buffers (mimics QueryResult.rowsRaw)
const result = {
  rowsRaw: [[Buffer.from([0x49])], [Buffer.from([0x50, 0x51])]],
  rowCount: 2,
  command: "SELECT",
};
console.log("nested:", JSON.stringify(result));

// Empty buffer
console.log("empty:", JSON.stringify(Buffer.alloc(0)));

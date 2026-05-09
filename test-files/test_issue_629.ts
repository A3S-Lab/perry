// Issue #629: namespace imports for unresolved modules used to produce
// `typeof fsp === "boolean"` (TAG_TRUE fallback) → confusing
// "(boolean).readFile is not a function" errors. Now resolves to an
// empty-object stub: typeof "object", missing properties cleanly read
// undefined. Real implementations will route through perry-stdlib /
// perry-ext-* bindings; this test only verifies the shape of the
// fallback.
import * as fsp from "node:fs/promises";
console.log("typeof fsp:", typeof fsp);
// Without a real implementation behind fs/promises, fsp.readFile reads
// undefined — same shape as `({} as any).readFile`. Perry's behavior
// diverges from Node here (Node has a real fs/promises module that
// resolves fully); the relevant fix is that `typeof fsp` is no longer
// "boolean".

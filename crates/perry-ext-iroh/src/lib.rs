//! Native bindings for [Iroh](https://www.iroh.computer/) — closes #425.
//!
//! Iroh is a Rust library for direct peer-to-peer QUIC
//! connections, with hole punching + relay fallback. The
//! wrapper exposes a TypeScript surface for opening an endpoint,
//! reading the local node id, and gracefully shutting down.
//!
//! # Status (v0.5.x — first cut)
//!
//! Minimum-viable port: `bind` / `nodeId` / `close`. Full
//! end-to-end peer connections (open_bi / read_to_end / event
//! callbacks) need:
//!
//! - perry-ffi closure invocation surface (✅ shipped in v0.5.542
//!   — used here for the would-be `onConnection` callback);
//! - long-lived per-stream handles (handle registry covers it,
//!   but the right ergonomic shape over async-streams takes a
//!   second pass);
//! - per-call ALPN bytes (we hardcode `"perry-iroh/0"` for the
//!   first cut so the FFI signatures stay simple).
//!
//! Followups land incrementally without breaking the FFI
//! signatures shipped here.
//!
//! # Why MVP scope
//!
//! Iroh is a substantial QUIC + hole-punching stack. The user-
//! visible Rust API is small (Endpoint::bind, connect, open_bi,
//! …) but a faithful TS-side API needs careful design — what's
//! a "connection" on the JS side, how do streams map to
//! Promises vs. AsyncIterables, who owns lifetime when a peer
//! disconnects, etc. Ship MVP now to satisfy #425 and validate
//! perry-ffi covers the basic surface; the richer API design
//! is a separate followup.

use perry_ffi::{
    drop_handle, register_handle, spawn_blocking, with_handle, Handle, JsPromise, JsValue, Promise,
};

use iroh::{endpoint::presets, Endpoint};

/// Wrapper struct so the registry's downcast resolves uniquely.
pub struct IrohEndpoint {
    pub endpoint: Endpoint,
}

/// `iroh.bind() -> Promise<Handle>` — bind a fresh QUIC endpoint
/// using Iroh's `N0` relay preset (sane defaults: discovery via
/// the n0 number-DNS, n0 relay servers for hole-punch fallback).
/// Resolves with an opaque integer handle.
#[no_mangle]
pub extern "C" fn js_iroh_bind() -> *mut Promise {
    let promise = JsPromise::new();
    let raw = promise.as_raw();

    spawn_blocking(move || {
        let result = tokio::runtime::Handle::current()
            .block_on(async move { Endpoint::bind(presets::N0).await });
        match result {
            Ok(endpoint) => {
                let handle = register_handle(IrohEndpoint { endpoint });
                promise.resolve(JsValue::from_number(handle as f64));
            }
            Err(e) => promise.reject_string(&format!("iroh bind: {}", e)),
        }
    });
    raw
}

/// `iroh.nodeId(handle) -> Promise<string>` — return the local
/// node's stable identifier (a hex-encoded Ed25519 public key).
/// This is what users share so peers can connect to them.
#[no_mangle]
pub extern "C" fn js_iroh_node_id(ep_handle: Handle) -> *mut Promise {
    let promise = JsPromise::new();
    let raw = promise.as_raw();

    spawn_blocking(move || {
        let result = with_handle::<IrohEndpoint, _, _>(ep_handle, |h| {
            tokio::runtime::Handle::current().block_on(async {
                // Wait for the endpoint to come online before
                // reading addr (it might not have a relay address
                // yet on a cold start).
                h.endpoint.online().await;
                h.endpoint.addr().id.to_string()
            })
        });
        match result {
            Some(id) => promise.resolve_string(&id),
            None => promise.reject_string("iroh: invalid endpoint handle"),
        }
    });
    raw
}

/// `iroh.close(handle) -> Promise<void>` — close the endpoint
/// gracefully. Drops the handle from the registry.
#[no_mangle]
pub extern "C" fn js_iroh_close(ep_handle: Handle) -> *mut Promise {
    let promise = JsPromise::new();
    let raw = promise.as_raw();

    spawn_blocking(move || {
        // Take the handle (consumes it) so we own the Endpoint
        // and can call `close().await`.
        let endpoint = perry_ffi::take_handle::<IrohEndpoint>(ep_handle);
        match endpoint {
            Some(h) => {
                tokio::runtime::Handle::current().block_on(async move {
                    h.endpoint.close().await;
                });
                promise.resolve_undefined();
            }
            None => {
                // Handle didn't exist — treat as no-op success
                // (idempotent close).
                drop_handle(ep_handle);
                promise.resolve_undefined();
            }
        }
    });
    raw
}

#[cfg(test)]
mod tests {
    // End-to-end iroh tests need a live tokio runtime + network
    // access (n0 relay + hole-punching infrastructure). Out of
    // scope for unit testing — the wrapper just plumbs through
    // the iroh crate's public methods, which have their own
    // upstream test coverage. Smoke testing happens via
    // TS integration in release builds.
    //
    // The pattern used here (handle registry + spawn_blocking +
    // tokio::Handle::current().block_on) mirrors
    // perry-ext-tursodb (#424) — both are validated end-to-end
    // through the same path.
}

//! Native bindings for the npm `axios` HTTP client.
//!
//! Phase 5 step 13 — first HTTP-client wrapper port. Uses
//! perry-ffi v0.5.x's full surface: handle registry +
//! spawn_blocking + JsPromise + JsValue. Reqwest under the hood
//! (same as perry-stdlib's existing axios copy).
//!
//! Functionally identical to `crates/perry-stdlib/src/axios.rs`.

use perry_ffi::{
    alloc_string, get_handle, json_stringify, read_string, register_handle, spawn_blocking,
    with_handle, Handle, JsPromise, JsString, JsValue, Promise, StringHeader,
};

/// #598: read the body argument as a JSON string. axios in npm-land
/// accepts the body as either a string (sent as-is) or any JS value
/// (JSON.stringify'd before send). Pre-fix Perry's FFI took a raw
/// `*const StringHeader`, which the codegen produced by unboxing the
/// caller's NaN-boxed value — for an object literal the unboxed
/// pointer was a real `*mut ObjectHeader`, the runtime read it as a
/// `*mut StringHeader`, and the request body became the byte pattern
/// of the ObjectHeader struct followed by the first character of the
/// stringified field. Same shape under bun: `axios.post(url, {a:1})`
/// sends `{"a":1}`. With the new f64 signature, the codegen passes
/// the NaN-boxed value through; here we route strings unchanged and
/// JSON.stringify everything else.
unsafe fn read_body_as_string(value_bits: f64) -> String {
    const STRING_TAG: u64 = 0x7FFF_0000_0000_0000;
    const SHORT_STRING_TAG: u64 = 0x7FFB_0000_0000_0000;
    const TAG_MASK: u64 = 0xFFFF_0000_0000_0000;
    const TAG_UNDEFINED: u64 = 0x7FFC_0000_0000_0001;
    const TAG_NULL: u64 = 0x7FFC_0000_0000_0002;
    let bits = value_bits.to_bits();
    if bits == TAG_UNDEFINED || bits == TAG_NULL {
        return String::new();
    }
    let tag = bits & TAG_MASK;
    if tag == STRING_TAG || tag == SHORT_STRING_TAG {
        // String: read as-is, no JSON quoting.
        let ptr = (bits & 0x0000_FFFF_FFFF_FFFF) as *const StringHeader;
        let handle = JsString::from_raw(ptr as *mut StringHeader);
        return read_string(handle).map(String::from).unwrap_or_default();
    }
    // Object / number / array / etc. — JSON.stringify.
    let v = JsValue::from_bits(bits);
    json_stringify(v).unwrap_or_default()
}

/// Response handle wrapper.
pub struct AxiosResponseHandle {
    pub status: u16,
    pub status_text: String,
    pub data: String,
}

unsafe fn read_str(ptr: *const StringHeader) -> Option<String> {
    let handle = JsString::from_raw(ptr as *mut StringHeader);
    read_string(handle).map(String::from)
}

/// Common request driver — runs the reqwest call inside
/// spawn_blocking, packages the response into an
/// `AxiosResponseHandle`, registers it, and resolves the promise
/// with a POINTER_TAG-tagged handle value (issue #340 trick from
/// the original perry-stdlib axios — without the explicit
/// NaN-boxing, the awaiter sees a subnormal float that decays
/// to `undefined` on `r.status` accesses).
fn run_request<F>(
    method: &'static str,
    url_or_err: Result<String, &'static str>,
    build: F,
) -> *mut Promise
where
    F: FnOnce(reqwest::Client, String) -> reqwest::RequestBuilder + Send + 'static,
{
    let promise = JsPromise::new();
    let raw = promise.as_raw();
    let url = match url_or_err {
        Ok(u) => u,
        Err(msg) => {
            promise.reject_string(msg);
            return raw;
        }
    };

    spawn_blocking(move || {
        let result: Result<AxiosResponseHandle, String> = tokio::runtime::Handle::current()
            .block_on(async move {
                let client = reqwest::Client::new();
                let request = build(client, url);
                let response = request
                    .send()
                    .await
                    .map_err(|e| format!("{} request failed: {}", method, e))?;
                let status = response.status().as_u16();
                let status_text = response
                    .status()
                    .canonical_reason()
                    .unwrap_or("")
                    .to_string();
                let data = response
                    .text()
                    .await
                    .map_err(|e| format!("Failed to read response body: {}", e))?;
                Ok(AxiosResponseHandle {
                    status,
                    status_text,
                    data,
                })
            });
        match result {
            Ok(resp) => {
                let handle = register_handle(resp);
                // POINTER_TAG-tagged handle value — see #340.
                promise.resolve(JsValue::from_object_ptr(handle as *mut ()));
            }
            Err(msg) => promise.reject_string(&msg),
        }
    });
    raw
}

/// `axios.get(url) -> Promise<Response>`.
///
/// # Safety
///
/// `url_ptr` must be null or a Perry-runtime `StringHeader`.
#[no_mangle]
pub unsafe extern "C" fn js_axios_get(url_ptr: *const StringHeader) -> *mut Promise {
    let url = read_str(url_ptr).ok_or("Invalid URL");
    run_request("GET", url, |client, url| client.get(&url))
}

/// `axios.post(url, data) -> Promise<Response>`.
///
/// # Safety
///
/// `url_ptr` must be null or a Perry-runtime `StringHeader`. `data` is
/// a NaN-boxed JSValue — strings are sent as-is, all other shapes are
/// JSON.stringify'd. See `read_body_as_string` for the routing rule
/// (#598). The signature uses `f64` to match the codegen dispatch's
/// pass-as-double path; Rust's calling convention puts it in d0 / a
/// vector register on AArch64, matching what the codegen emits.
#[no_mangle]
pub unsafe extern "C" fn js_axios_post(
    url_ptr: *const StringHeader,
    data: f64,
) -> *mut Promise {
    let url = read_str(url_ptr).ok_or("Invalid URL");
    let body = read_body_as_string(data);
    run_request("POST", url, move |client, url| {
        client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
    })
}

/// `axios.put(url, data) -> Promise<Response>`. Same body-encoding
/// rule as `axios.post` (#598).
///
/// # Safety
///
/// `url_ptr` must be null or a Perry-runtime `StringHeader`. `data` is
/// a NaN-boxed JSValue.
#[no_mangle]
pub unsafe extern "C" fn js_axios_put(
    url_ptr: *const StringHeader,
    data: f64,
) -> *mut Promise {
    let url = read_str(url_ptr).ok_or("Invalid URL");
    let body = read_body_as_string(data);
    run_request("PUT", url, move |client, url| {
        client
            .put(&url)
            .header("Content-Type", "application/json")
            .body(body)
    })
}

/// `axios.delete(url) -> Promise<Response>`.
///
/// # Safety
///
/// `url_ptr` must be null or a Perry-runtime `StringHeader`.
#[no_mangle]
pub unsafe extern "C" fn js_axios_delete(url_ptr: *const StringHeader) -> *mut Promise {
    let url = read_str(url_ptr).ok_or("Invalid URL");
    run_request("DELETE", url, |client, url| client.delete(&url))
}

/// `axios.patch(url, data) -> Promise<Response>`. Same body-encoding
/// rule as `axios.post` (#598).
///
/// # Safety
///
/// `url_ptr` must be null or a Perry-runtime `StringHeader`. `data` is
/// a NaN-boxed JSValue.
#[no_mangle]
pub unsafe extern "C" fn js_axios_patch(
    url_ptr: *const StringHeader,
    data: f64,
) -> *mut Promise {
    let url = read_str(url_ptr).ok_or("Invalid URL");
    let body = read_body_as_string(data);
    run_request("PATCH", url, move |client, url| {
        client
            .patch(&url)
            .header("Content-Type", "application/json")
            .body(body)
    })
}

/// `response.status -> number`.
#[no_mangle]
pub extern "C" fn js_axios_response_status(handle: Handle) -> f64 {
    if let Some(r) = get_handle::<AxiosResponseHandle>(handle) {
        r.status as f64
    } else {
        0.0
    }
}

/// `response.statusText -> string`.
#[no_mangle]
pub extern "C" fn js_axios_response_status_text(handle: Handle) -> *mut StringHeader {
    with_handle::<AxiosResponseHandle, _, _>(handle, |r| alloc_string(&r.status_text).as_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `response.data -> string`.
#[no_mangle]
pub extern "C" fn js_axios_response_data(handle: Handle) -> *mut StringHeader {
    with_handle::<AxiosResponseHandle, _, _>(handle, |r| alloc_string(&r.data).as_raw())
        .unwrap_or(std::ptr::null_mut())
}

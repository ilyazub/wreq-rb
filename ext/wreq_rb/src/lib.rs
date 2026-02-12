use rb_sys::*;
use std::ffi::{c_int, c_long, c_ulong, CString, CStr};
use std::os::raw::c_char;
use std::ptr;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use lazy_static::lazy_static;
use tokio::runtime::Runtime;
use url::Url;
use wreq::redirect::Policy;
use wreq::header::{HeaderMap, HeaderName, HeaderValue};
use base64::Engine;

// Global references to Ruby classes
static mut RB_HTTP_CLIENT_CLASS: VALUE = 0;
static mut RB_HTTP_RESPONSE_CLASS: VALUE = 0;

type WreqError = wreq::Error;
type WreqResponse = wreq::Response;
type WreqEmulation = wreq_util::Emulation;
type WreqEmulationOption = wreq_util::EmulationOption;

unsafe fn is_hash(v: VALUE) -> bool {
    RB_TYPE(v) == ruby_value_type::RUBY_T_HASH
}

// Emulation helpers
fn build_emulation_option(emulation: WreqEmulation) -> WreqEmulationOption {
    match emulation {
        WreqEmulation::SafariIos17_4_1
        | WreqEmulation::SafariIos17_2
        | WreqEmulation::SafariIos16_5 => WreqEmulationOption::builder()
            .emulation(emulation)
            .emulation_os(wreq_util::EmulationOS::IOS)
            .build(),
        WreqEmulation::FirefoxAndroid135 => WreqEmulationOption::builder()
            .emulation(emulation)
            .emulation_os(wreq_util::EmulationOS::Android)
            .build(),
        _ => WreqEmulationOption::builder().emulation(emulation).build(),
    }
}

fn get_random_emulation() -> WreqEmulationOption {
    let options = [
        WreqEmulation::Chrome134,
        WreqEmulation::Chrome128,
        WreqEmulation::Firefox135,
        WreqEmulation::Safari17_0,
        WreqEmulation::SafariIos17_4_1,
        WreqEmulation::FirefoxAndroid135,
    ];
    let index = (fast_random() as usize) % options.len();
    build_emulation_option(options[index])
}

fn get_random_desktop_emulation() -> WreqEmulationOption {
    let options = [
        WreqEmulation::Chrome134,
        WreqEmulation::Chrome128,
        WreqEmulation::Firefox135,
        WreqEmulation::Safari17_0,
    ];
    let index = (fast_random() as usize) % options.len();
    build_emulation_option(options[index])
}

fn get_random_mobile_emulation() -> WreqEmulationOption {
    let options = [
        WreqEmulation::SafariIos17_4_1,
        WreqEmulation::SafariIos17_2,
        WreqEmulation::SafariIos16_5,
        WreqEmulation::FirefoxAndroid135,
    ];
    let index = (fast_random() as usize) % options.len();
    build_emulation_option(options[index])
}

fn fast_random() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap_or_default();
    let time = since_the_epoch.as_nanos() as u64;
    let increment = COUNTER.fetch_add(1, Ordering::Relaxed);
    time.wrapping_add(increment)
}

macro_rules! ffi_guard {
    ($body:block) => {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)) {
            Ok(v) => v,
            Err(_) => {
                let msg = std::ffi::CString::new("Rust panic caught").unwrap();
                unsafe { rb_raise(rb_eRuntimeError, msg.as_ptr()) };
                Qnil as VALUE
            }
        }
    }
}

// Helper to convert Ruby string/symbol to Rust String
unsafe fn ruby_to_string(value: VALUE) -> Result<String, String> {
    if value == Qnil as VALUE {
        return Err("Expected string, got nil".to_string());
    }
    let type_ = RB_TYPE(value);
    if type_ == ruby_value_type::RUBY_T_STRING {
        let mut val = value;
        let c_str = rb_string_value_cstr(&mut val);
        let rust_str = CStr::from_ptr(c_str).to_str()
            .map_err(|e| format!("UTF-8 error: {}", e))?;
        Ok(rust_str.to_string())
    } else if type_ == ruby_value_type::RUBY_T_SYMBOL {
        let id = rb_sym2id(value);
        let name_ptr = rb_id2name(id);
        if name_ptr.is_null() {
            return Err("Symbol conversion failed".to_string());
        }
        let rust_str = CStr::from_ptr(name_ptr).to_str()
            .map_err(|e| format!("UTF-8 error: {}", e))?;
        Ok(rust_str.to_string())
    } else {
        let mut val = rb_obj_as_string(value);
        let c_str = rb_string_value_cstr(&mut val);
        let rust_str = CStr::from_ptr(c_str).to_str()
            .map_err(|e| format!("UTF-8 error: {}", e))?;
        Ok(rust_str.to_string())
    }
}

// Helper to get a value from a hash by string key (handles string or symbol keys)
unsafe fn hash_get(hash: VALUE, key: &str) -> Option<VALUE> {
    let key_str = CString::new(key).unwrap();
    let key_sym = rb_intern(key_str.as_ptr());
    
    let val = rb_hash_lookup(hash, rb_id2sym(key_sym));
    if val != Qnil as VALUE {
        return Some(val);
    }
    
    let val = rb_hash_lookup(hash, rb_str_new_cstr(key_str.as_ptr()));
    if val != Qnil as VALUE {
        return Some(val);
    }
    
    None
}

fn wreq_error_to_string(err: WreqError) -> String {
    format!("HTTP request failed: {}", err)
}

fn normalize_header_name(name: &str) -> String {
    name.replace('_', "-")
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

lazy_static! {
    static ref RUNTIME: Arc<Runtime> = Arc::new(
        Runtime::new().expect("Failed to create tokio runtime")
    );
}

fn get_runtime() -> Result<Arc<Runtime>, String> {
    Ok(Arc::clone(&RUNTIME))
}


// Helper to convert Ruby hash to Rust HashMap (simple string keys/values)
unsafe fn ruby_hash_to_map(hash: VALUE) -> Result<HashMap<String, String>, String> {
    if RB_TYPE(hash) != ruby_value_type::RUBY_T_HASH {
        return Err("Expected hash".to_string());
    }

    struct HashData {
        map: HashMap<String, String>,
        error: Option<String>,
    }

    unsafe extern "C" fn callback(key: VALUE, val: VALUE, arg: VALUE) -> c_int {
        unsafe {
            let data = &mut *(arg as *mut HashData);
            let key_res = ruby_to_string(key);
            let val_res = ruby_to_string(val);

            match (key_res, val_res) {
                (Ok(k), Ok(v)) => {
                    data.map.insert(k, v);
                    0 // Continue
                }
                (Err(e), _) | (_, Err(e)) => {
                    data.error = Some(e);
                    1 // Stop
                }
            }
        }
    }

    let mut data = HashData {
        map: HashMap::new(),
        error: None,
    };

    rb_hash_foreach(
        hash,
        Some(std::mem::transmute(callback as *const ())),
        &mut data as *mut HashData as VALUE
    );

    if let Some(err) = data.error {
        Err(err)
    } else {
        Ok(data.map)
    }
}

unsafe fn ruby_value_to_json(value: VALUE) -> Result<serde_json::Value, String> {
    let value_type = RB_TYPE(value);
    
    match value_type {
        ruby_value_type::RUBY_T_NIL => Ok(serde_json::Value::Null),
        ruby_value_type::RUBY_T_TRUE => Ok(serde_json::Value::Bool(true)),
        ruby_value_type::RUBY_T_FALSE => Ok(serde_json::Value::Bool(false)),
        ruby_value_type::RUBY_T_FIXNUM => Ok(serde_json::json!(rb_num2long(value))),
        ruby_value_type::RUBY_T_FLOAT => Ok(serde_json::json!(rb_float_value(value))),
        ruby_value_type::RUBY_T_STRING => {
            let mut val = value;
            let c_str = rb_string_value_cstr(&mut val);
            let rust_str = CStr::from_ptr(c_str).to_str()
                .map_err(|e| format!("UTF-8 error: {}", e))?;
            Ok(serde_json::Value::String(rust_str.to_string()))
        }
        ruby_value_type::RUBY_T_SYMBOL => {
            let id = rb_sym2id(value);
            let name_ptr = rb_id2name(id);
            if name_ptr.is_null() {
                return Err("Symbol conversion failed".to_string());
            }
            let rust_str = CStr::from_ptr(name_ptr).to_str()
                .map_err(|e| format!("UTF-8 error: {}", e))?;
            Ok(serde_json::Value::String(rust_str.to_string()))
        }
        ruby_value_type::RUBY_T_ARRAY => {
            let len = RARRAY_LEN(value);
            let mut arr = Vec::with_capacity(len as usize);
            for i in 0..len {
                let elem = rb_ary_entry(value, i);
                arr.push(ruby_value_to_json(elem)?);
            }
            Ok(serde_json::Value::Array(arr))
        }
        ruby_value_type::RUBY_T_HASH => {
            struct HashData {
                map: serde_json::Map<String, serde_json::Value>,
                error: Option<String>,
            }
            
    unsafe extern "C" fn callback(key: VALUE, val: VALUE, arg: VALUE) -> c_int {
        unsafe {
            let data = &mut *(arg as *mut HashData);
            let key_str = match ruby_value_to_json(key) {
                        Ok(serde_json::Value::String(s)) => s,
                        Ok(_) => {
                            data.error = Some("Hash key not string".to_string());
                            return 1;
                        }
                        Err(e) => {
                            data.error = Some(e);
                            return 1;
                        }
                    };
                    match ruby_value_to_json(val) {
                        Ok(json_val) => {
                            data.map.insert(key_str, json_val);
                            0
                        }
                        Err(e) => {
                            data.error = Some(e);
                            1
                        }
                    }
                }
            }
            
            let mut data = HashData {
                map: serde_json::Map::new(),
                error: None,
            };
            
            rb_hash_foreach(
                value,
                Some(std::mem::transmute(callback as *const ())),
                &mut data as *mut HashData as VALUE
            );
            
            if let Some(err) = data.error {
                return Err(err);
            }
            
            Ok(serde_json::Value::Object(data.map))
        }
        _ => Err(format!("Unsupported type: {:?}", value_type))
    }
}

struct RequestOptions {
    body: Option<String>,
    content_type: Option<String>,
}

unsafe fn extract_options(args: &[VALUE]) -> Result<RequestOptions, String> {
    if args.len() <= 1 {
        return Ok(RequestOptions {
            body: None,
            content_type: None,
        });
    }

    let opts_value = args[1];
    
    // Check if it's a hash
    if RB_TYPE(opts_value) != ruby_value_type::RUBY_T_HASH {
         return Ok(RequestOptions {
            body: Some(ruby_to_string(opts_value)?),
            content_type: None,
        });
    }

    // It is a hash
    let opts_hash = opts_value;
    
    if let Some(json_val) = hash_get(opts_hash, "json") {
        let json_value = ruby_value_to_json(json_val)?;
        let json_str = serde_json::to_string(&json_value)
            .map_err(|e| format!("JSON error: {}", e))?;
        return Ok(RequestOptions {
            body: Some(json_str),
            content_type: Some("application/json".to_string()),
        });
    }
    
    if let Some(form_val) = hash_get(opts_hash, "form") {
        if RB_TYPE(form_val) == ruby_value_type::RUBY_T_HASH {
            let map = ruby_hash_to_map(form_val)?;
             let mut pairs = Vec::new();
             for (k, v) in map {
                 pairs.push(format!("{}={}", 
                     urlencoding::encode(&k),
                     urlencoding::encode(&v)
                 ));
             }
            
            return Ok(RequestOptions {
                body: Some(pairs.join("&")),
                content_type: Some("application/x-www-form-urlencoded".to_string()),
            });
        }
    }
    
    if let Some(body_val) = hash_get(opts_hash, "body") {
         return Ok(RequestOptions {
            body: Some(ruby_to_string(body_val)?),
            content_type: Some("text/plain; charset=utf-8".to_string()),
        });
    }
    
    Ok(RequestOptions {
        body: None,
        content_type: None,
    })
}

unsafe fn apply_params_to_url(url_str: &str, args: &[VALUE]) -> Result<String, String> {
    if args.len() <= 1 {
        return Ok(url_str.to_string());
    }
    
    let opts_value = args[1];
    if RB_TYPE(opts_value) == ruby_value_type::RUBY_T_HASH {
         if let Some(params_val) = hash_get(opts_value, "params") {
             if RB_TYPE(params_val) == ruby_value_type::RUBY_T_HASH {
                 let mut url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;
                 let map = ruby_hash_to_map(params_val)?;
                 {
                    let mut query_pairs = url.query_pairs_mut();
                    for (k, v) in map {
                        query_pairs.append_pair(&k, &v);
                    }
                 }
                 return Ok(url.to_string());
             }
         }
    }
    
    Ok(url_str.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Patch,
    Options,
}

fn execute_request(
    client: &wreq::Client,
    method: HttpMethod,
    url: &str,
    headers: &HashMap<String, String>,
    user_agent: &Option<String>,
    redirect_policy: &Option<Policy>,
    timeout: f64,
    body: Option<String>,
    content_type: Option<String>,
) -> Result<RbHttpResponse, String> {
    let runtime = get_runtime()?;

    let mut request = match method {
        HttpMethod::Get => client.get(url),
        HttpMethod::Post => client.post(url),
        HttpMethod::Put => client.put(url),
        HttpMethod::Delete => client.delete(url),
        HttpMethod::Head => client.head(url),
        HttpMethod::Patch => client.patch(url),
        HttpMethod::Options => client.options(url),
    };

    // Pre-allocate HeaderMap with capacity (headers + 3 defaults: accept, user-agent, content-type)
    let mut header_map = HeaderMap::with_capacity(headers.len() + 3);
    let mut orig_headers = wreq::header::OrigHeaderMap::with_capacity(headers.len() + 3);

    // Track which special headers were set by the user
    let mut has_accept = false;
    let mut has_user_agent = false;
    let mut has_content_type = false;

    // Process user-provided headers
    for (key, value) in headers {
        let key_lower = key.to_lowercase();
        
        // Check if this is a special header we handle separately
        match key_lower.as_str() {
            "accept" => {
                has_accept = true;
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(key.as_bytes()),
                    HeaderValue::from_str(value),
                ) {
                    header_map.insert(name, val);
                }
                orig_headers.insert(key.clone());
            }
            "user-agent" => {
                has_user_agent = true;
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(key.as_bytes()),
                    HeaderValue::from_str(value),
                ) {
                    header_map.insert(name, val);
                }
                orig_headers.insert(key.clone());
            }
            "content-type" => {
                has_content_type = true;
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(key.as_bytes()),
                    HeaderValue::from_str(value),
                ) {
                    header_map.insert(name, val);
                }
                orig_headers.insert(key.clone());
            }
            _ => {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(key.as_bytes()),
                    HeaderValue::from_str(value),
                ) {
                    header_map.insert(name, val);
                }
                orig_headers.insert(key.clone());
            }
        }
    }

    let default_headers_enabled = true;
    if default_headers_enabled && !has_accept {
        header_map.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("*/*"),
        );
        orig_headers.insert("accept");
    }

    // Set user-agent header if provided and not already set
    if default_headers_enabled {
        if let Some(ua) = user_agent {
            if !has_user_agent {
                if let Ok(val) = HeaderValue::from_str(ua) {
                    header_map.insert(HeaderName::from_static("user-agent"), val);
                }
                orig_headers.insert("user-agent");
            }
        }
    }

    // Set content-type header if provided and not already set
    if let Some(ct) = &content_type {
        if !has_content_type {
            if let Ok(val) = HeaderValue::from_str(ct) {
                header_map.insert(HeaderName::from_static("content-type"), val);
            }
            orig_headers.insert("content-type");
        }
    }
    // Don't automatically set application/octet-stream - let the server handle defaults

    request = request
        .headers(header_map)
        .orig_headers(orig_headers)
        .default_headers(default_headers_enabled);

    if let Some(policy) = redirect_policy {
        request = request.redirect(policy.clone());
    }

    if timeout > 0.0 {
        request = request.timeout(Duration::from_secs_f64(timeout));
    }

    if let Some(body_str) = body {
        request = request.body(body_str);
    }

    let response = runtime
        .block_on(request.send())
        .map_err(wreq_error_to_string)?;

    runtime.block_on(RbHttpResponse::new(response))
}

struct ClientWrap(wreq::Client);

impl ClientWrap {
    fn inner(&self) -> &wreq::Client {
        &self.0
    }
}

impl Clone for ClientWrap {
    fn clone(&self) -> Self {
        ClientWrap(self.0.clone())
    }
}

// TypedData for RbHttpClient (rb-sys migration)
unsafe extern "C" fn client_free(data: *mut std::ffi::c_void) {
    if !data.is_null() {
        unsafe {
            drop(Box::from_raw(data as *mut RbHttpClient));
        }
    }
}

unsafe extern "C" fn client_size(_data: *const std::ffi::c_void) -> std::ffi::c_ulong {
    std::mem::size_of::<RbHttpClient>() as std::ffi::c_ulong
}

static RB_HTTP_CLIENT_TYPE: RbDataTypeWrapper = RbDataTypeWrapper(rb_data_type_t {
    wrap_struct_name: c"RbHttpClient".as_ptr() as *const c_char,
    function: rb_data_type_struct__bindgen_ty_1 {
        dfree: Some(client_free),
        dsize: Some(client_size),
        dmark: None,
        dcompact: None,
        reserved: [std::ptr::null_mut(); 1],
    },
    parent: std::ptr::null(),
    data: std::ptr::null_mut(),
    flags: RUBY_TYPED_FREE_IMMEDIATELY as VALUE,
});

fn get_client_type() -> &'static rb_data_type_t {
    &RB_HTTP_CLIENT_TYPE.0
}

// Helper functions for wrapping/unwrapping RbHttpClient
unsafe fn wrap_client(client: RbHttpClient) -> VALUE {
    let boxed = Box::new(client);
    let ptr = Box::into_raw(boxed) as *mut std::ffi::c_void;
    let class = unsafe { RB_HTTP_CLIENT_CLASS };
    rb_data_typed_object_wrap(
        class,
        ptr,
        get_client_type()
    )
}

unsafe fn unwrap_client(value: VALUE) -> *mut RbHttpClient {
    let ptr = rb_check_typeddata(
        value,
        get_client_type()
    );
    if ptr.is_null() {
        rb_raise(rb_eTypeError, CString::new("Expected RbHttpClient").unwrap().as_ptr());
    }
    ptr as *mut RbHttpClient
}

struct RbHttpClient {
    client: ClientWrap,
    headers: HashMap<String, String>,
    user_agent: Option<String>,
    redirect_policy: Option<Policy>,
    timeout: f64,
    proxy: Option<String>,
    // Future http.rb feature scaffolding (Tasks 4-11)
    cookies: Option<HashMap<String, String>>,
    auth_header: Option<String>,
    accept_type: Option<String>,
    encoding: Option<String>,
    base_url: Option<String>,
    closed: AtomicBool,
}

impl RbHttpClient {
    fn new() -> Result<Self, String> {
        let client = wreq::Client::builder()
            .emulation(get_random_emulation())
            .build()
            .map_err(|e| format!("Failed to create client: {}", e))?;

        Ok(Self {
            client: ClientWrap(client),
            headers: HashMap::new(),
            user_agent: None,
            redirect_policy: Some(Policy::limited(10)),
            timeout: 0.0,
            proxy: None,
            cookies: None,
            auth_header: None,
            accept_type: None,
            encoding: None,
            base_url: None,
            closed: AtomicBool::new(false),
        })
    }

    fn new_desktop() -> Result<Self, String> {
        let client = wreq::Client::builder()
            .emulation(get_random_desktop_emulation())
            .build()
            .map_err(|e| format!("Failed to create client: {}", e))?;

        Ok(Self {
            client: ClientWrap(client),
            headers: HashMap::new(),
            user_agent: None,
            redirect_policy: Some(Policy::limited(10)),
            timeout: 0.0,
            proxy: None,
            cookies: None,
            auth_header: None,
            accept_type: None,
            encoding: None,
            base_url: None,
            closed: AtomicBool::new(false),
        })
    }

    fn new_mobile() -> Result<Self, String> {
        let client = wreq::Client::builder()
            .emulation(get_random_mobile_emulation())
            .build()
            .map_err(|e| format!("Failed to create client: {}", e))?;

        Ok(Self {
            client: ClientWrap(client),
            headers: HashMap::new(),
            user_agent: None,
            redirect_policy: Some(Policy::limited(10)),
            timeout: 0.0,
            proxy: None,
            cookies: None,
            auth_header: None,
            accept_type: None,
            encoding: None,
            base_url: None,
            closed: AtomicBool::new(false),
        })
    }

    fn ensure_open(&self) -> Result<(), String> {
        if self.closed.load(Ordering::SeqCst) {
            return Err("HTTP client is closed".to_string());
        }

        Ok(())
    }

    fn resolve_url(&self, url_str: &str) -> Result<String, String> {
        if let Ok(parsed) = Url::parse(url_str) {
            return Ok(parsed.to_string());
        }

        if let Some(base_url) = &self.base_url {
            let base = Url::parse(base_url).map_err(|e| format!("Invalid base URL: {}", e))?;
            let joined = base.join(url_str).map_err(|e| format!("Invalid URL: {}", e))?;
            return Ok(joined.to_string());
        }

        Err("Relative URL requires base URL".to_string())
    }

    fn with_headers(&self, headers: HashMap<String, String>) -> Self {
        let mut new_client = self.clone();
        new_client.headers.clear();

        for (name, value) in headers {
            let normalized_key = normalize_header_name(&name);
            new_client.headers.insert(normalized_key, value);
        }
        new_client
    }

    fn with_proxy(&self, proxy: String) -> Result<Self, String> {
        let mut new_client = self.clone();
        new_client.proxy = Some(proxy.clone());

        let client = wreq::Client::builder()
            .emulation(get_random_emulation())
            .proxy(wreq::Proxy::all(&proxy).map_err(|e| format!("Invalid proxy URL: {}", e))?)
            .build()
            .map_err(|e| format!("Failed to create client with proxy: {}", e))?;

        new_client.client = ClientWrap(client);

        Ok(new_client)
    }

    unsafe fn follow(&self, args: &[VALUE]) -> Result<Self, String> {
        let mut new_client = self.clone();
        
        if args.is_empty() {
            new_client.redirect_policy = Some(Policy::limited(10));
        } else {
            let arg = args[0];
            if RB_TYPE(arg) == ruby_value_type::RUBY_T_TRUE {
                 new_client.redirect_policy = Some(Policy::limited(10));
            } else if RB_TYPE(arg) == ruby_value_type::RUBY_T_FALSE {
                 new_client.redirect_policy = Some(Policy::none());
            } else if is_hash(arg) {
                if let Some(max_hops_val) = hash_get(arg, "max_hops") {
                    let max_hops = rb_num2long(max_hops_val) as usize;
                    new_client.redirect_policy = Some(Policy::limited(max_hops));
                } else {
                    new_client.redirect_policy = Some(Policy::limited(10));
                }
            } else {
                return Err("follow() requires bool or hash with :max_hops".to_string());
            }
        }
        
        Ok(new_client)
    }

    unsafe fn persistent(&self, args: &[VALUE]) -> Result<Self, String> {
        let host = ruby_to_string(args[0])?;
        let base_url = Url::parse(&host).map_err(|e| format!("Invalid base URL: {}", e))?;

        let mut new_client = self.clone();
        new_client.base_url = Some(base_url.to_string());

        if args.len() > 1 {
            let opts_value = args[1];
            if is_hash(opts_value) {
                if let Some(timeout_val) = hash_get(opts_value, "timeout") {
                    let timeout = rb_num2dbl(timeout_val);
                    new_client.timeout = timeout;
                }
            }
        }

        Ok(new_client)
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    fn timeout(&self, secs: f64) -> Self {
        let mut new_client = self.clone();
        new_client.timeout = secs;
        new_client
    }

    unsafe fn via(&self, args: &[VALUE]) -> Result<Self, String> {
        let host = ruby_to_string(args[0])?;
        let port = rb_num2long(args[1]);
        
        let proxy_url = if args.len() >= 4 {
            let user = ruby_to_string(args[2])?;
            let pass = ruby_to_string(args[3])?;
            format!("http://{}:{}@{}:{}", user, pass, host, port)
        } else {
            format!("http://{}:{}", host, port)
        };
        
        self.with_proxy(proxy_url)
    }

    unsafe fn cookies(&self, cookies_hash: VALUE) -> Result<Self, String> {
        let mut new_client = self.clone();
        let map = ruby_hash_to_map(cookies_hash)?;
        let mut cookie_pairs = Vec::new();
        
        for (k, v) in map {
            cookie_pairs.push(format!("{}={}", k, v));
        }
        
        let cookie_string = cookie_pairs.join("; ");
        new_client.headers.insert("Cookie".to_string(), cookie_string);
        Ok(new_client)
    }

    unsafe fn basic_auth(&self, auth_hash: VALUE) -> Result<Self, String> {
        if !is_hash(auth_hash) {
            return Err("basic_auth requires a Hash".to_string());
        }

        let user = hash_get(auth_hash, "user")
            .ok_or_else(|| "basic_auth requires :user".to_string())?;
        let pass = hash_get(auth_hash, "pass")
            .ok_or_else(|| "basic_auth requires :pass".to_string())?;
        
        let user_str = ruby_to_string(user)?;
        let pass_str = ruby_to_string(pass)?;
        
        let credentials = format!("{}:{}", user_str, pass_str);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        
        let mut new_client = self.clone();
        new_client.headers.insert("Authorization".to_string(), format!("Basic {}", encoded));
        Ok(new_client)
    }

    fn auth(&self, auth_value: String) -> Self {
        let mut new_client = self.clone();
        new_client.headers.insert("Authorization".to_string(), auth_value);
        new_client
    }

    unsafe fn accept(&self, accept_value: VALUE) -> Result<Self, String> {
        let mut new_client = self.clone();
        
        let accept_header = if RB_TYPE(accept_value) == ruby_value_type::RUBY_T_SYMBOL {
             let id = rb_sym2id(accept_value);
             let name_ptr = rb_id2name(id);
             let name_str = if name_ptr.is_null() {
                 ""
             } else {
                 CStr::from_ptr(name_ptr).to_str().unwrap_or("")
             };
             
            match name_str {
                "json" => "application/json",
                "xml" => "application/xml",
                "html" => "text/html",
                "text" => "text/plain",
                _ => return Err(format!("Unknown accept type: {}", name_str)),
            }
        } else {
            &ruby_to_string(accept_value)?
        };
        
        new_client.headers.insert("Accept".to_string(), accept_header.to_string());
        Ok(new_client)
    }

    fn encoding(&self, enc: String) -> Self {
        let mut new_client = self.clone();
        new_client.encoding = Some(enc);
        new_client
    }

    unsafe fn get(&self, args: &[VALUE]) -> Result<RbHttpResponse, String> {
        self.ensure_open()?;
        let url_str = ruby_to_string(args[0])?;
        let resolved_url = self.resolve_url(&url_str)?;
        let url = apply_params_to_url(&resolved_url, args)?;
        let opts = extract_options(args)?;
        
        execute_request(
            self.client.inner(),
            HttpMethod::Get,
            &url,
            &self.headers,
            &self.user_agent,
            &self.redirect_policy,
            self.timeout,
            opts.body,
            opts.content_type,
        )
    }

    unsafe fn post(&self, args: &[VALUE]) -> Result<RbHttpResponse, String> {
        self.ensure_open()?;
        let url_str = ruby_to_string(args[0])?;
        let resolved_url = self.resolve_url(&url_str)?;
        let url = apply_params_to_url(&resolved_url, args)?;
        let opts = extract_options(args)?;

        execute_request(
            self.client.inner(),
            HttpMethod::Post,
            &url,
            &self.headers,
            &self.user_agent,
            &self.redirect_policy,
            self.timeout,
            opts.body,
            opts.content_type,
        )
    }

    unsafe fn put(&self, args: &[VALUE]) -> Result<RbHttpResponse, String> {
        self.ensure_open()?;
        let url_str = ruby_to_string(args[0])?;
        let resolved_url = self.resolve_url(&url_str)?;
        let url = apply_params_to_url(&resolved_url, args)?;
        let opts = extract_options(args)?;

        execute_request(
            self.client.inner(),
            HttpMethod::Put,
            &url,
            &self.headers,
            &self.user_agent,
            &self.redirect_policy,
            self.timeout,
            opts.body,
            opts.content_type,
        )
    }

    unsafe fn delete(&self, args: &[VALUE]) -> Result<RbHttpResponse, String> {
        self.ensure_open()?;
        let url_str = ruby_to_string(args[0])?;
        let resolved_url = self.resolve_url(&url_str)?;
        let url = apply_params_to_url(&resolved_url, args)?;
        let opts = extract_options(args)?;
        
        execute_request(
            self.client.inner(),
            HttpMethod::Delete,
            &url,
            &self.headers,
            &self.user_agent,
            &self.redirect_policy,
            self.timeout,
            opts.body,
            opts.content_type,
        )
    }

    unsafe fn head(&self, args: &[VALUE]) -> Result<RbHttpResponse, String> {
        self.ensure_open()?;
        let url_str = ruby_to_string(args[0])?;
        let resolved_url = self.resolve_url(&url_str)?;
        let url = apply_params_to_url(&resolved_url, args)?;
        let opts = extract_options(args)?;
        
        execute_request(
            self.client.inner(),
            HttpMethod::Head,
            &url,
            &self.headers,
            &self.user_agent,
            &self.redirect_policy,
            self.timeout,
            opts.body,
            opts.content_type,
        )
    }

    unsafe fn patch(&self, args: &[VALUE]) -> Result<RbHttpResponse, String> {
        self.ensure_open()?;
        let url_str = ruby_to_string(args[0])?;
        let resolved_url = self.resolve_url(&url_str)?;
        let url = apply_params_to_url(&resolved_url, args)?;
        let opts = extract_options(args)?;

        execute_request(
            self.client.inner(),
            HttpMethod::Patch,
            &url,
            &self.headers,
            &self.user_agent,
            &self.redirect_policy,
            self.timeout,
            opts.body,
            opts.content_type,
        )
    }

    unsafe fn request(&self, args: &[VALUE]) -> Result<RbHttpResponse, String> {
        self.ensure_open()?;
        let verb = ruby_to_string(args[0])?;
        let verb_str: &str = &verb;
        let method = match verb_str {
            "get" => HttpMethod::Get,
            "post" => HttpMethod::Post,
            "put" => HttpMethod::Put,
            "delete" => HttpMethod::Delete,
            "head" => HttpMethod::Head,
            "patch" => HttpMethod::Patch,
            "options" => HttpMethod::Options,
            _ => return Err("Invalid HTTP verb".to_string()),
        };
        
        let url_str = ruby_to_string(args[1])?;
        let resolved_url = self.resolve_url(&url_str)?;
        let url = apply_params_to_url(&resolved_url, &args[1..])?;
        let opts = extract_options(&args[1..])?;
        
        execute_request(
            self.client.inner(),
            method,
            &url,
            &self.headers,
            &self.user_agent,
            &self.redirect_policy,
            self.timeout,
            opts.body,
            opts.content_type,
        )
    }

    unsafe fn headers(&self, headers_hash: VALUE) -> Result<Self, String> {
        let headers = ruby_hash_to_map(headers_hash)?;
        Ok(self.with_headers(headers))
    }
}

impl Clone for RbHttpClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            headers: self.headers.clone(),
            user_agent: self.user_agent.clone(),
            redirect_policy: self.redirect_policy.clone(),
            timeout: self.timeout,
            proxy: self.proxy.clone(),
            cookies: self.cookies.clone(),
            auth_header: self.auth_header.clone(),
            accept_type: self.accept_type.clone(),
            encoding: self.encoding.clone(),
            base_url: self.base_url.clone(),
            closed: AtomicBool::new(self.closed.load(Ordering::Relaxed)),
        }
    }
}

struct ResponseData {
    status: u16,
    headers: HashMap<String, String>,
    body: Option<String>,
    url: String,
}

// TypedData for RbHttpResponse (rb-sys migration)
unsafe extern "C" fn response_free(data: *mut std::ffi::c_void) {
    if !data.is_null() {
        unsafe {
            drop(Box::from_raw(data as *mut RbHttpResponse));
        }
    }
}

unsafe extern "C" fn response_size(_data: *const std::ffi::c_void) -> c_ulong {
    std::mem::size_of::<RbHttpResponse>() as c_ulong
}

// Wrapper type to allow static rb_data_type_t which contains non-Sync pointers
struct RbDataTypeWrapper(rb_data_type_t);

// SAFETY: This is safe to share between threads as it's only ever read
// and contains constant data (function pointers and null pointers)
unsafe impl Sync for RbDataTypeWrapper {}

static RB_HTTP_RESPONSE_TYPE: RbDataTypeWrapper = RbDataTypeWrapper(rb_data_type_t {
    wrap_struct_name: c"RbHttpResponse".as_ptr() as *const c_char,
    function: rb_data_type_struct__bindgen_ty_1 {
        dfree: Some(response_free),
        dsize: Some(response_size),
        dmark: None,
        dcompact: None,
        reserved: [std::ptr::null_mut(); 1],
    },
    parent: std::ptr::null(),
    data: std::ptr::null_mut(),
    flags: RUBY_TYPED_FREE_IMMEDIATELY as VALUE,
});

fn get_response_type() -> &'static rb_data_type_t {
    &RB_HTTP_RESPONSE_TYPE.0
}


struct RbHttpResponse {
    data: Arc<ResponseData>,
}

impl RbHttpResponse {
    async fn new(response: WreqResponse) -> Result<Self, String> {
        let status = response.status().as_u16();
        let url = response.uri().to_string();

        let mut headers = HashMap::new();
        for (name, value) in response.headers().iter() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        let body = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                eprintln!("Warning: Failed to read response body: {}", e);
                String::new()
            }
        };

        Ok(Self {
            data: Arc::new(ResponseData {
                status,
                headers,
                body: Some(body),
                url,
            }),
        })
    }

    fn status(&self) -> u16 {
        self.data.status
    }

    fn body(&self) -> String {
        match &self.data.body {
            Some(body) => body.clone(),
            None => String::new(),
        }
    }

    fn to_s(&self) -> String {
        self.body()
    }

    fn headers(&self) -> HashMap<String, String> {
        self.data.headers.clone()
    }

    fn content_type(&self) -> Option<String> {
        self.data.headers.get("content-type").cloned()
    }

    fn uri(&self) -> String {
        self.data.url.clone()
    }

    fn code(&self) -> u16 {
        self.data.status
    }

    fn charset(&self) -> Option<String> {
        if let Some(content_type) = self.content_type() {
            if let Some(charset_part) = content_type
                .split(';')
                .skip(1)
                .find(|part| part.trim().to_lowercase().starts_with("charset="))
            {
                let charset = charset_part
                    .trim()
                    .split('=')
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_string();

                if !charset.is_empty() {
                    return Some(charset);
                }
            }
        }
        None
    }
}

// Helper functions for wrapping/unwrapping RbHttpResponse
unsafe fn wrap_response(response: RbHttpResponse) -> VALUE {
    let boxed = Box::new(response);
    let ptr = Box::into_raw(boxed) as *mut std::ffi::c_void;
    let class = unsafe { RB_HTTP_RESPONSE_CLASS };
    rb_data_typed_object_wrap(
        class,
        ptr,
        get_response_type()
    )
}

unsafe fn unwrap_response(value: VALUE) -> *mut RbHttpResponse {
    let ptr = rb_check_typeddata(
        value,
        get_response_type()
    );
    if ptr.is_null() {
        rb_raise(rb_eTypeError, CString::new("Expected RbHttpResponse").unwrap().as_ptr());
    }
    ptr as *mut RbHttpResponse
}

unsafe fn rb_get(args: &[VALUE]) -> Result<RbHttpResponse, String> {
    let client = RbHttpClient::new()?;
    client.get(args)
}

fn rb_desktop() -> Result<RbHttpClient, String> {
    RbHttpClient::new_desktop()
}

fn rb_mobile() -> Result<RbHttpClient, String> {
    RbHttpClient::new_mobile()
}

unsafe fn rb_post(args: &[VALUE]) -> Result<RbHttpResponse, String> {
    let client = RbHttpClient::new()?;
    client.post(args)
}

unsafe fn rb_put(args: &[VALUE]) -> Result<RbHttpResponse, String> {
    let client = RbHttpClient::new()?;
    client.put(args)
}

unsafe fn rb_delete(args: &[VALUE]) -> Result<RbHttpResponse, String> {
    let client = RbHttpClient::new()?;
    client.delete(args)
}

unsafe fn rb_head(args: &[VALUE]) -> Result<RbHttpResponse, String> {
    let client = RbHttpClient::new()?;
    client.head(args)
}

unsafe fn rb_patch(args: &[VALUE]) -> Result<RbHttpResponse, String> {
    let client = RbHttpClient::new()?;
    client.patch(args)
}

unsafe fn rb_request(args: &[VALUE]) -> Result<RbHttpResponse, String> {
    let client = RbHttpClient::new()?;
    client.request(args)
}

unsafe fn rb_persistent(args: &[VALUE]) -> Result<RbHttpClient, String> {
    RbHttpClient::new()?.persistent(args)
}

unsafe fn rb_headers(headers_hash: VALUE) -> Result<RbHttpClient, String> {
    let client = RbHttpClient::new()?;
    client.headers(headers_hash)
}

unsafe fn rb_follow(args: &[VALUE]) -> Result<RbHttpClient, String> {
    RbHttpClient::new()?.follow(args)
}

fn rb_timeout(secs: f64) -> Result<RbHttpClient, String> {
    Ok(RbHttpClient::new()?.timeout(secs))
}

fn rb_proxy(proxy: String) -> Result<RbHttpClient, String> {
    RbHttpClient::new()?.with_proxy(proxy)
}

unsafe fn rb_via(args: &[VALUE]) -> Result<RbHttpClient, String> {
    RbHttpClient::new()?.via(args)
}

unsafe fn rb_cookies(cookies_hash: VALUE) -> Result<RbHttpClient, String> {
    Ok(RbHttpClient::new()?.cookies(cookies_hash)?)
}

unsafe fn rb_basic_auth(auth_hash: VALUE) -> Result<RbHttpClient, String> {
    RbHttpClient::new()?.basic_auth(auth_hash)
}

fn rb_auth(auth_value: String) -> Result<RbHttpClient, String> {
    Ok(RbHttpClient::new()?.auth(auth_value))
}

fn rb_encoding(enc: String) -> Result<RbHttpClient, String> {
    Ok(RbHttpClient::new()?.encoding(enc))
}

unsafe fn rb_accept(accept_value: VALUE) -> Result<RbHttpClient, String> {
    RbHttpClient::new()?.accept(accept_value)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_response_status(self_val: VALUE) -> VALUE {
    ffi_guard!({
        let response_ptr = unwrap_response(self_val);
        let response = &*response_ptr;
        let status = response.status();
        rb_int2inum(status as isize)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_response_code(self_val: VALUE) -> VALUE {
    ffi_guard!({
        let response_ptr = unwrap_response(self_val);
        let response = &*response_ptr;
        let code = response.code();
        rb_int2inum(code as isize)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_response_body(self_val: VALUE) -> VALUE {
    ffi_guard!({
        let response_ptr = unwrap_response(self_val);
        let response = &*response_ptr;
        let body = response.body();
        let c_str = std::ffi::CString::new(body).unwrap_or_default();
        rb_str_new_cstr(c_str.as_ptr())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_response_to_s(self_val: VALUE) -> VALUE {
    ffi_guard!({
        let response_ptr = unwrap_response(self_val);
        let response = &*response_ptr;
        let s = response.to_s();
        let c_str = std::ffi::CString::new(s).unwrap_or_default();
        rb_str_new_cstr(c_str.as_ptr())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_response_uri(self_val: VALUE) -> VALUE {
    ffi_guard!({
        let response_ptr = unwrap_response(self_val);
        let response = &*response_ptr;
        let uri = response.uri();
        let c_str = std::ffi::CString::new(uri).unwrap_or_default();
        rb_str_new_cstr(c_str.as_ptr())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_response_content_type(self_val: VALUE) -> VALUE {
    ffi_guard!({
        let response_ptr = unwrap_response(self_val);
        let response = &*response_ptr;
        match response.content_type() {
            Some(ct) => {
                let c_str = std::ffi::CString::new(ct).unwrap_or_default();
                rb_str_new_cstr(c_str.as_ptr())
            }
            None => Qnil as VALUE
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_response_charset(self_val: VALUE) -> VALUE {
    ffi_guard!({
        let response_ptr = unwrap_response(self_val);
        let response = &*response_ptr;
        match response.charset() {
            Some(cs) => {
                let c_str = std::ffi::CString::new(cs).unwrap_or_default();
                rb_str_new_cstr(c_str.as_ptr())
            }
            None => Qnil as VALUE
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_response_headers(self_val: VALUE) -> VALUE {
    ffi_guard!({
        let response_ptr = unwrap_response(self_val);
        let response = &*response_ptr;
        let headers_map = response.headers();
        
        let hash = rb_hash_new();
        for (key, value) in headers_map.iter() {
            let k_cstr = std::ffi::CString::new(key.as_str()).unwrap_or_default();
            let v_cstr = std::ffi::CString::new(value.as_str()).unwrap_or_default();
            let k_val = rb_str_new_cstr(k_cstr.as_ptr());
            let v_val = rb_str_new_cstr(v_cstr.as_ptr());
            rb_hash_aset(hash, k_val, v_val);
        }
        hash
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_new(_class: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => wrap_client(client),
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create HTTP client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                unreachable!()
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_new_desktop(_class: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new_desktop() {
            Ok(client) => wrap_client(client),
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create desktop HTTP client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                unreachable!()
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_new_mobile(_class: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new_mobile() {
            Ok(client) => wrap_client(client),
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create mobile HTTP client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                unreachable!()
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_close(self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        client.close();
        Qnil as VALUE
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_with_headers(self_val: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        match ruby_hash_to_map(arg) {
            Ok(headers) => {
                let new_client = client.with_headers(headers);
                wrap_client(new_client)
            }
            Err(_) => {
                let msg = std::ffi::CString::new("with_headers() requires a Hash").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_timeout(self_val: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        let secs = rb_num2dbl(arg);
        let new_client = client.timeout(secs);
        wrap_client(new_client)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_with_proxy(self_val: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        match ruby_to_string(arg) {
            Ok(proxy) => {
                match client.with_proxy(proxy) {
                    Ok(new_client) => wrap_client(new_client),
                    Err(_) => {
                        let msg = std::ffi::CString::new("with_proxy() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("with_proxy() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_cookies(self_val: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        match client.cookies(arg) {
            Ok(new_client) => wrap_client(new_client),
            Err(_) => {
                let msg = std::ffi::CString::new("cookies() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
     })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_basic_auth(self_val: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        match client.basic_auth(arg) {
            Ok(new_client) => wrap_client(new_client),
            Err(_) => {
                let msg = std::ffi::CString::new("basic_auth() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_auth(self_val: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        match ruby_to_string(arg) {
            Ok(auth_value) => {
                let new_client = client.auth(auth_value);
                wrap_client(new_client)
            }
            Err(_) => {
                let msg = std::ffi::CString::new("auth() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_accept(self_val: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        match client.accept(arg) {
            Ok(new_client) => wrap_client(new_client),
            Err(_) => {
                let msg = std::ffi::CString::new("accept() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_encoding(self_val: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        match ruby_to_string(arg) {
            Ok(enc) => {
                let new_client = client.encoding(enc);
                wrap_client(new_client)
            }
            Err(_) => {
                let msg = std::ffi::CString::new("encoding() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_headers(self_val: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        match ruby_hash_to_map(arg) {
            Ok(headers) => {
                let new_client = client.with_headers(headers);
                wrap_client(new_client)
            }
            Err(_) => {
                let msg = std::ffi::CString::new("headers() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_follow(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.follow(args) {
            Ok(new_client) => wrap_client(new_client),
            Err(_) => {
                let msg = std::ffi::CString::new("follow() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_via(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.via(args) {
            Ok(new_client) => wrap_client(new_client),
            Err(_) => {
                let msg = std::ffi::CString::new("via() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_get(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.get(args) {
            Ok(response) => wrap_response(response),
            Err(_) => {
                let msg = std::ffi::CString::new("get() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_post(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.post(args) {
            Ok(response) => wrap_response(response),
            Err(_) => {
                let msg = std::ffi::CString::new("post() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_put(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.put(args) {
            Ok(response) => wrap_response(response),
            Err(_) => {
                let msg = std::ffi::CString::new("put() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
     })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_delete(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.delete(args) {
            Ok(response) => wrap_response(response),
            Err(_) => {
                let msg = std::ffi::CString::new("delete() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_head(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.head(args) {
            Ok(response) => wrap_response(response),
            Err(_) => {
                let msg = std::ffi::CString::new("head() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_patch(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.patch(args) {
            Ok(response) => wrap_response(response),
            Err(_) => {
                let msg = std::ffi::CString::new("patch() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_request(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.request(args) {
            Ok(response) => wrap_response(response),
            Err(e) => {
                let exc = if e == "Invalid HTTP verb" {
                    rb_eArgError
                } else {
                    rb_eRuntimeError
                };
                let msg = std::ffi::CString::new("request() failed").unwrap();
                rb_raise(exc, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_client_persistent(argc: c_int, argv: *const VALUE, self_val: VALUE) -> VALUE {
    ffi_guard!({
        let client_ptr = unwrap_client(self_val);
        let client = &mut *client_ptr;
        
        let args = if argc > 0 {
            std::slice::from_raw_parts(argv, argc as usize)
        } else {
            &[]
        };
        
        match client.persistent(args) {
            Ok(new_client) => wrap_client(new_client),
            Err(_) => {
                let msg = std::ffi::CString::new("persistent() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_get(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.get(args) {
                    Ok(result) => wrap_response(result),
                    Err(_) => {
                        let msg = std::ffi::CString::new("get() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_desktop(_self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new_desktop() {
            Ok(client) => wrap_client(client),
            Err(_) => {
                let msg = std::ffi::CString::new("desktop() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_mobile(_self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new_mobile() {
            Ok(client) => wrap_client(client),
            Err(_) => {
                let msg = std::ffi::CString::new("mobile() failed").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_post(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.post(args) {
                    Ok(result) => wrap_response(result),
                    Err(_) => {
                        let msg = std::ffi::CString::new("post() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_put(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.put(args) {
                    Ok(result) => wrap_response(result),
                    Err(_) => {
                        let msg = std::ffi::CString::new("put() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_delete(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.delete(args) {
                    Ok(result) => wrap_response(result),
                    Err(_) => {
                        let msg = std::ffi::CString::new("delete() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_head(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.head(args) {
                    Ok(result) => wrap_response(result),
                    Err(_) => {
                        let msg = std::ffi::CString::new("head() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_patch(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.patch(args) {
                    Ok(result) => wrap_response(result),
                    Err(_) => {
                        let msg = std::ffi::CString::new("patch() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_request(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.request(args) {
                    Ok(result) => wrap_response(result),
                    Err(e) => {
                        let exc = if e == "Invalid HTTP verb" {
                            rb_eArgError
                        } else {
                            rb_eRuntimeError
                        };
                        let msg = std::ffi::CString::new("request() failed").unwrap();
                        rb_raise(exc, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_persistent(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.persistent(args) {
                    Ok(result) => wrap_client(result),
                    Err(_) => {
                        let msg = std::ffi::CString::new("persistent() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_headers(_self: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                match ruby_hash_to_map(arg) {
                    Ok(headers) => {
                        let new_client = client.with_headers(headers);
                        wrap_client(new_client)
                    }
                    Err(_) => {
                        let msg = std::ffi::CString::new("headers() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_follow(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.follow(args) {
                    Ok(result) => wrap_client(result),
                    Err(_) => {
                        let msg = std::ffi::CString::new("follow() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_timeout(_self: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let secs = rb_num2dbl(arg);
                let new_client = client.timeout(secs);
                wrap_client(new_client)
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_proxy(_self: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                match ruby_to_string(arg) {
                    Ok(proxy_str) => {
                        match client.with_proxy(proxy_str) {
                            Ok(new_client) => wrap_client(new_client),
                            Err(_) => {
                                let msg = std::ffi::CString::new("proxy() failed").unwrap();
                                rb_raise(rb_eRuntimeError, msg.as_ptr());
                                Qnil as VALUE
                            }
                        }
                    }
                    Err(_) => {
                        let msg = std::ffi::CString::new("proxy() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_via(argc: c_int, argv: *const VALUE, _self: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                let args = if argc > 0 {
                    std::slice::from_raw_parts(argv, argc as usize)
                } else {
                    &[]
                };
                match client.via(args) {
                    Ok(result) => wrap_client(result),
                    Err(_) => {
                        let msg = std::ffi::CString::new("via() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_cookies(_self: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                match client.cookies(arg) {
                    Ok(new_client) => wrap_client(new_client),
                    Err(_) => {
                        let msg = std::ffi::CString::new("cookies() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_basic_auth(_self: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                match client.basic_auth(arg) {
                    Ok(new_client) => wrap_client(new_client),
                    Err(_) => {
                        let msg = std::ffi::CString::new("basic_auth() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_auth(_self: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                match ruby_to_string(arg) {
                    Ok(auth_str) => {
                         let new_client = client.auth(auth_str);
                         wrap_client(new_client)
                    }
                    Err(_) => {
                        let msg = std::ffi::CString::new("auth() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_accept(_self: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                match client.accept(arg) {
                    Ok(new_client) => wrap_client(new_client),
                    Err(_) => {
                        let msg = std::ffi::CString::new("accept() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_http_encoding(_self: VALUE, arg: VALUE) -> VALUE {
    ffi_guard!({
        match RbHttpClient::new() {
            Ok(client) => {
                match ruby_to_string(arg) {
                    Ok(enc_str) => {
                        let new_client = client.encoding(enc_str);
                        wrap_client(new_client)
                    }
                    Err(_) => {
                        let msg = std::ffi::CString::new("encoding() failed").unwrap();
                        rb_raise(rb_eRuntimeError, msg.as_ptr());
                        Qnil as VALUE
                    }
                }
            }
            Err(_) => {
                let msg = std::ffi::CString::new("Failed to create client").unwrap();
                rb_raise(rb_eRuntimeError, msg.as_ptr());
                Qnil as VALUE
            }
        }
    })
}

// Raw rb-sys Init function
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Init_wreq_rb() {
    let wreq_module = unsafe { rb_define_module(c"Wreq".as_ptr()) };
    let http_module = unsafe { rb_define_module_under(wreq_module, c"HTTP".as_ptr()) };
    
    // Response class definition
    let response_class = unsafe { rb_define_class_under(http_module, c"Response".as_ptr(), rb_cObject) };
    unsafe { RB_HTTP_RESPONSE_CLASS = response_class };
    unsafe { rb_define_method(response_class, c"status".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_response_status as *const ())), 0) };
    unsafe { rb_define_method(response_class, c"body".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_response_body as *const ())), 0) };
    unsafe { rb_define_method(response_class, c"to_s".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_response_to_s as *const ())), 0) };
    unsafe { rb_define_method(response_class, c"headers".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_response_headers as *const ())), 0) };
    unsafe { rb_define_method(response_class, c"content_type".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_response_content_type as *const ())), 0) };
    unsafe { rb_define_method(response_class, c"uri".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_response_uri as *const ())), 0) };
    unsafe { rb_define_method(response_class, c"code".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_response_code as *const ())), 0) };
     unsafe { rb_define_method(response_class, c"charset".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_response_charset as *const ())), 0) };

    // Client class definition
    let client_class = unsafe { rb_define_class_under(http_module, c"Client".as_ptr(), rb_cObject) };
    unsafe { RB_HTTP_CLIENT_CLASS = client_class };
    
    // Singleton methods (arity 0)
    unsafe { rb_define_singleton_method(client_class, c"new".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_new as *const ())), 0) };
    unsafe { rb_define_singleton_method(client_class, c"new_desktop".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_new_desktop as *const ())), 0) };
    unsafe { rb_define_singleton_method(client_class, c"new_mobile".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_new_mobile as *const ())), 0) };
    
    // Instance methods - arity 1
    unsafe { rb_define_method(client_class, c"with_headers".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_with_headers as *const ())), 1) };
    unsafe { rb_define_method(client_class, c"timeout".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_timeout as *const ())), 1) };
    unsafe { rb_define_method(client_class, c"with_proxy".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_with_proxy as *const ())), 1) };
    unsafe { rb_define_method(client_class, c"cookies".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_cookies as *const ())), 1) };
    unsafe { rb_define_method(client_class, c"basic_auth".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_basic_auth as *const ())), 1) };
    unsafe { rb_define_method(client_class, c"auth".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_auth as *const ())), 1) };
    unsafe { rb_define_method(client_class, c"accept".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_accept as *const ())), 1) };
    unsafe { rb_define_method(client_class, c"encoding".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_encoding as *const ())), 1) };
    unsafe { rb_define_method(client_class, c"headers".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_headers as *const ())), 1) };
    
    // Instance methods - arity -1 (variable args)
    unsafe { rb_define_method(client_class, c"follow".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_follow as *const ())), -1) };
    unsafe { rb_define_method(client_class, c"via".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_via as *const ())), -1) };
    unsafe { rb_define_method(client_class, c"get".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_get as *const ())), -1) };
    unsafe { rb_define_method(client_class, c"post".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_post as *const ())), -1) };
    unsafe { rb_define_method(client_class, c"put".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_put as *const ())), -1) };
    unsafe { rb_define_method(client_class, c"delete".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_delete as *const ())), -1) };
    unsafe { rb_define_method(client_class, c"head".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_head as *const ())), -1) };
    unsafe { rb_define_method(client_class, c"patch".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_patch as *const ())), -1) };
    unsafe { rb_define_method(client_class, c"request".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_request as *const ())), -1) };
    unsafe { rb_define_method(client_class, c"persistent".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_persistent as *const ())), -1) };
    
     // Instance method - arity 0
     unsafe { rb_define_method(client_class, c"close".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_client_close as *const ())), 0) };

     // HTTP module functions
     unsafe { rb_define_module_function(http_module, c"get".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_get as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"desktop".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_desktop as *const ())), 0) };
     unsafe { rb_define_module_function(http_module, c"mobile".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_mobile as *const ())), 0) };
     unsafe { rb_define_module_function(http_module, c"post".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_post as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"put".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_put as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"delete".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_delete as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"head".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_head as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"patch".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_patch as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"request".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_request as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"persistent".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_persistent as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"headers".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_headers as *const ())), 1) };
     unsafe { rb_define_module_function(http_module, c"follow".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_follow as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"timeout".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_timeout as *const ())), 1) };
     unsafe { rb_define_module_function(http_module, c"proxy".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_proxy as *const ())), 1) };
     unsafe { rb_define_module_function(http_module, c"via".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_via as *const ())), -1) };
     unsafe { rb_define_module_function(http_module, c"cookies".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_cookies as *const ())), 1) };
     unsafe { rb_define_module_function(http_module, c"basic_auth".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_basic_auth as *const ())), 1) };
     unsafe { rb_define_module_function(http_module, c"auth".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_auth as *const ())), 1) };
     unsafe { rb_define_module_function(http_module, c"accept".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_accept as *const ())), 1) };
     unsafe { rb_define_module_function(http_module, c"encoding".as_ptr(), Some(std::mem::transmute::<*const (), unsafe extern "C" fn() -> VALUE>(rb_http_encoding as *const ())), 1) };
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for fast_random() - pure function returning u64
    #[test]
    fn test_fast_random_returns_non_zero() {
        let value = fast_random();
        assert_ne!(value, 0, "fast_random should not return zero");
    }

    #[test]
    fn test_fast_random_varies() {
        let value1 = fast_random();
        let value2 = fast_random();
        assert_ne!(
            value1, value2,
            "fast_random should return different values on successive calls"
        );
    }

    #[test]
    fn test_get_random_desktop_emulation_valid() {
        let _emulation = get_random_desktop_emulation();
    }

    #[test]
    fn test_get_random_mobile_emulation_valid() {
        let _emulation = get_random_mobile_emulation();
    }

    #[test]
    fn test_get_random_emulation_valid() {
        let _emulation = get_random_emulation();
    }

    // Tests for normalize_header_name() - multiple test cases
    #[test]
    fn test_normalize_header_name_underscores() {
        let result = normalize_header_name("content_type");
        assert_eq!(result, "Content-Type");
    }

    #[test]
    fn test_normalize_header_name_hyphens() {
        let result = normalize_header_name("x-custom-header");
        assert_eq!(result, "X-Custom-Header");
    }

    #[test]
    fn test_normalize_header_name_uppercase() {
        let result = normalize_header_name("ACCEPT");
        assert_eq!(result, "Accept");
    }

    #[test]
    fn test_normalize_header_name_mixed() {
        let result = normalize_header_name("content_type");
        assert_eq!(result, "Content-Type");
    }

    #[test]
    fn test_normalize_header_name_already_normalized() {
        let result = normalize_header_name("Content-Type");
        assert_eq!(result, "Content-Type");
    }

    // Tests for HttpMethod enum - Copy/Clone traits
    #[test]
    fn test_http_method_copy() {
        let method1 = HttpMethod::Get;
        let method2 = method1; // Copy should work
        assert!(matches!(method1, HttpMethod::Get));
        assert!(matches!(method2, HttpMethod::Get));
    }

    #[test]
    fn test_http_method_clone() {
        let method1 = HttpMethod::Post;
        let method2 = method1.clone();
        assert!(matches!(method1, HttpMethod::Post));
        assert!(matches!(method2, HttpMethod::Post));
    }

    #[test]
    fn test_http_method_equality() {
        let get1 = HttpMethod::Get;
        let get2 = HttpMethod::Get;
        let post = HttpMethod::Post;
        assert_eq!(get1, get2);
        assert_ne!(get1, post);
    }
}

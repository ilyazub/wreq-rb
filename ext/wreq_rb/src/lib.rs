use rb_sys::*;
use std::ffi::{c_int, c_long, CString, CStr};
use std::os::raw::c_char;
use std::ptr;
use magnus::r_hash::ForEach;
use magnus::{
    Error as MagnusError, IntoValue, Module, Object, RHash, Symbol, TryConvert, Value, exception,
    function, method,
};
use wreq::header::{HeaderMap, HeaderName, HeaderValue};
use wreq::redirect::Policy;
use wreq::{Error as WreqError, Response as WreqResponse};
use wreq_util::Emulation as WreqEmulation;
use std::cell::Cell;
use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::num::Wrapping;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::runtime::Runtime;
use url::Url;
use lazy_static::lazy_static;

// Fast random implementation similar to wreq-util crate
fn fast_random() -> u64 {
    thread_local! {
        static RNG: Cell<Wrapping<u64>> = Cell::new(Wrapping(seed()));
    }

    #[inline]
    fn seed() -> u64 {
        let seed = RandomState::new();
        let mut out = 0;
        let mut cnt = 0;
        while out == 0 {
            cnt += 1;
            let mut hasher = seed.build_hasher();
            hasher.write_usize(cnt);
            out = hasher.finish();
        }
        out
    }

    RNG.with(|rng| {
        let mut n = rng.get();
        debug_assert_ne!(n.0, 0);
        n ^= n >> 12;
        n ^= n << 25;
        n ^= n >> 27;
        rng.set(n);
        n.0.wrapping_mul(0x2545f4914f6cdd1d)
    })
}

fn get_random_desktop_emulation() -> WreqEmulation {
    let browsers = [
        WreqEmulation::Chrome134,
        WreqEmulation::Chrome128,
        WreqEmulation::Chrome101,
        WreqEmulation::Firefox135,
        WreqEmulation::Safari17_0,
    ];

    let index = (fast_random() as usize) % browsers.len();
    browsers[index]
}

fn get_random_mobile_emulation() -> WreqEmulation {
    let browsers = [
        WreqEmulation::SafariIos17_4_1,
        WreqEmulation::SafariIos17_2,
        WreqEmulation::SafariIos16_5,
        WreqEmulation::FirefoxAndroid135,
    ];

    let index = (fast_random() as usize) % browsers.len();
    browsers[index]
}

fn get_random_emulation() -> WreqEmulation {
    if fast_random() % 100 < 50 {
        get_random_desktop_emulation()
    } else {
        get_random_mobile_emulation()
    }
}

fn wreq_error_to_magnus_error(err: WreqError) -> MagnusError {
    MagnusError::new(
        exception::runtime_error(),
        format!("HTTP request failed: {}", err),
    )
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

fn get_runtime() -> Result<Arc<Runtime>, MagnusError> {
    Ok(Arc::clone(&RUNTIME))
}

struct RequestOptions {
    body: Option<String>,
    content_type: Option<String>,
}

fn extract_options(args: &[Value]) -> Result<RequestOptions, MagnusError> {
    if args.len() <= 1 {
        return Ok(RequestOptions {
            body: None,
            content_type: None,
        });
    }

    let opts_value = &args[1];
    if let Ok(opts_hash) = RHash::try_convert(*opts_value) {
        let json_key = Symbol::new("json").into_value();
        let form_key = Symbol::new("form").into_value();
        let body_key = Symbol::new("body").into_value();
        
        if let Some(json_val) = opts_hash.get(json_key) {
            let json_str = magnus::eval::<String>(&format!(
                "require 'json'; JSON.generate({})",
                json_val
            ))?;
            return Ok(RequestOptions {
                body: Some(json_str),
                content_type: Some("application/json".to_string()),
            });
        }
        
        if let Some(form_val) = opts_hash.get(form_key) {
            if let Ok(form_hash) = RHash::try_convert(form_val) {
                let mut pairs = Vec::new();
                form_hash.foreach(|key: Value, value: Value| {
                    let key_str = if let Some(sym) = Symbol::from_value(key) {
                        sym.name()?.to_string()
                    } else {
                        String::try_convert(key)?
                    };
                    let val_str = if let Some(sym) = Symbol::from_value(value) {
                        sym.name()?.to_string()
                    } else {
                        String::try_convert(value)?
                    };
                    pairs.push(format!("{}={}", 
                        urlencoding::encode(&key_str),
                        urlencoding::encode(&val_str)
                    ));
                    Ok(magnus::r_hash::ForEach::Continue)
                }).ok();
                
                return Ok(RequestOptions {
                    body: Some(pairs.join("&")),
                    content_type: Some("application/x-www-form-urlencoded".to_string()),
                });
            }
        }
        
        if let Some(body_val) = opts_hash.get(body_key) {
            if let Ok(body_str) = String::try_convert(body_val) {
                return Ok(RequestOptions {
                    body: Some(body_str),
                    content_type: Some("text/plain; charset=utf-8".to_string()),
                });
            }
        }
        
        Ok(RequestOptions {
            body: None,
            content_type: None,
        })
    } else {
        Ok(RequestOptions {
            body: Some(String::try_convert(*opts_value)?),
            content_type: None,
        })
    }
}

fn apply_params_to_url(url_str: &str, args: &[Value]) -> Result<String, MagnusError> {
    if args.len() <= 1 {
        return Ok(url_str.to_string());
    }
    
    if let Ok(opts_hash) = RHash::try_convert(args[1]) {
        let params_key = Symbol::new("params").into_value();
        if let Some(params_val) = opts_hash.get(params_key) {
            if let Ok(params_hash) = RHash::try_convert(params_val) {
                let mut url = Url::parse(url_str).map_err(|e| {
                    MagnusError::new(exception::arg_error(), format!("Invalid URL: {}", e))
                })?;
                
                {
                    let mut query_pairs = url.query_pairs_mut();
                    params_hash.foreach(|key: Value, value: Value| {
                        let key_str = if let Some(sym) = Symbol::from_value(key) {
                            sym.name()?.to_string()
                        } else {
                            String::try_convert(key)?
                        };
                        let val_str = if let Some(sym) = Symbol::from_value(value) {
                            sym.name()?.to_string()
                        } else {
                            String::try_convert(value)?
                        };
                        query_pairs.append_pair(&key_str, &val_str);
                        Ok(magnus::r_hash::ForEach::Continue)
                    }).ok();
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
) -> Result<RbHttpResponse, MagnusError> {
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
            }
            "user-agent" => {
                has_user_agent = true;
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(key.as_bytes()),
                    HeaderValue::from_str(value),
                ) {
                    header_map.insert(name, val);
                }
            }
            "content-type" => {
                has_content_type = true;
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(key.as_bytes()),
                    HeaderValue::from_str(value),
                ) {
                    header_map.insert(name, val);
                }
            }
            _ => {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(key.as_bytes()),
                    HeaderValue::from_str(value),
                ) {
                    header_map.insert(name, val);
                }
            }
        }
    }

    // Set accept header if not provided by user
    if !has_accept {
        header_map.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static("*/*"),
        );
    }

    // Set user-agent header if provided and not already set
    if let Some(ua) = user_agent {
        if !has_user_agent {
            if let Ok(val) = HeaderValue::from_str(ua) {
                header_map.insert(HeaderName::from_static("user-agent"), val);
            }
        }
    }

    // Set content-type header if provided and not already set
    if let Some(ct) = &content_type {
        if !has_content_type {
            if let Ok(val) = HeaderValue::from_str(ct) {
                header_map.insert(HeaderName::from_static("content-type"), val);
            }
        }
    }
    // Don't automatically set application/octet-stream - let the server handle defaults

    request = request.headers(header_map);

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
        .map_err(wreq_error_to_magnus_error)?;

    runtime.block_on(RbHttpResponse::new(response))
}

#[magnus::wrap(class = "Wreq::HTTP::Client")]
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

#[magnus::wrap(class = "Wreq::HTTP::Client")]
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
    fn new() -> Result<Self, MagnusError> {
        let client = wreq::Client::builder()
            .emulation(get_random_emulation())
            .build()
            .map_err(|e| {
                MagnusError::new(
                    exception::runtime_error(),
                    format!("Failed to create client: {}", e),
                )
            })?;

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

    fn new_desktop() -> Result<Self, MagnusError> {
        let client = wreq::Client::builder()
            .emulation(get_random_desktop_emulation())
            .build()
            .map_err(|e| {
                MagnusError::new(
                    exception::runtime_error(),
                    format!("Failed to create client: {}", e),
                )
            })?;

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

    fn new_mobile() -> Result<Self, MagnusError> {
        let client = wreq::Client::builder()
            .emulation(get_random_mobile_emulation())
            .build()
            .map_err(|e| {
                MagnusError::new(
                    exception::runtime_error(),
                    format!("Failed to create client: {}", e),
                )
            })?;

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

    fn ensure_open(&self) -> Result<(), MagnusError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(MagnusError::new(
                exception::runtime_error(),
                "HTTP client is closed",
            ));
        }

        Ok(())
    }

    fn resolve_url(&self, url_str: &str) -> Result<String, MagnusError> {
        if let Ok(parsed) = Url::parse(url_str) {
            return Ok(parsed.to_string());
        }

        if let Some(base_url) = &self.base_url {
            let base = Url::parse(base_url).map_err(|e| {
                MagnusError::new(
                    exception::arg_error(),
                    format!("Invalid base URL: {}", e),
                )
            })?;
            let joined = base.join(url_str).map_err(|e| {
                MagnusError::new(
                    exception::arg_error(),
                    format!("Invalid URL: {}", e),
                )
            })?;
            return Ok(joined.to_string());
        }

        Err(MagnusError::new(
            exception::runtime_error(),
            "Relative URL requires base URL",
        ))
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

    fn with_proxy(&self, proxy: String) -> Result<Self, MagnusError> {
        let mut new_client = self.clone();
        new_client.proxy = Some(proxy.clone());

        let client = wreq::Client::builder()
            .emulation(get_random_emulation())
            .proxy(wreq::Proxy::all(&proxy).map_err(|e| {
                MagnusError::new(
                    exception::runtime_error(),
                    format!("Invalid proxy URL: {}", e),
                )
            })?)
            .build()
            .map_err(|e| {
                MagnusError::new(
                    exception::runtime_error(),
                    format!("Failed to create client with proxy: {}", e),
                )
            })?;

        new_client.client = ClientWrap(client);

        Ok(new_client)
    }

    fn follow(&self, args: &[Value]) -> Result<Self, MagnusError> {
        let mut new_client = self.clone();
        
        if args.is_empty() {
            new_client.redirect_policy = Some(Policy::limited(10));
        } else {
            let arg = args[0];
            if let Some(bool_val) = bool::try_convert(arg).ok() {
                if bool_val {
                    new_client.redirect_policy = Some(Policy::limited(10));
                } else {
                    new_client.redirect_policy = Some(Policy::none());
                }
            } else if let Ok(hash) = RHash::try_convert(arg) {
                let max_hops_key = Symbol::new("max_hops").into_value();
                if let Some(max_hops_val) = hash.get(max_hops_key) {
                    let max_hops = usize::try_convert(max_hops_val)?;
                    new_client.redirect_policy = Some(Policy::limited(max_hops));
                } else {
                    new_client.redirect_policy = Some(Policy::limited(10));
                }
            } else {
                return Err(MagnusError::new(
                    exception::arg_error(),
                    "follow() requires bool or hash with :max_hops"
                ));
            }
        }
        
        Ok(new_client)
    }

    fn persistent(&self, args: &[Value]) -> Result<Self, MagnusError> {
        let host = String::try_convert(args[0])?;
        let base_url = Url::parse(&host).map_err(|e| {
            MagnusError::new(
                exception::arg_error(),
                format!("Invalid base URL: {}", e),
            )
        })?;

        let mut new_client = self.clone();
        new_client.base_url = Some(base_url.to_string());

        if args.len() > 1 {
            if let Ok(opts_hash) = RHash::try_convert(args[1]) {
                let timeout_key = Symbol::new("timeout").into_value();
                if let Some(timeout_val) = opts_hash.get(timeout_key) {
                    let timeout = f64::try_convert(timeout_val)?;
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

    fn via(&self, args: &[Value]) -> Result<Self, MagnusError> {
        let host = String::try_convert(args[0])?;
        let port = u16::try_convert(args[1])?;
        
        let proxy_url = if args.len() >= 4 {
            let user = String::try_convert(args[2])?;
            let pass = String::try_convert(args[3])?;
            format!("http://{}:{}@{}:{}", user, pass, host, port)
        } else {
            format!("http://{}:{}", host, port)
        };
        
        self.with_proxy(proxy_url)
    }

    fn cookies(&self, cookies_hash: RHash) -> Self {
        let mut new_client = self.clone();
        let mut cookie_pairs = Vec::new();
        
        cookies_hash.foreach(|key: Symbol, value: String| {
            cookie_pairs.push(format!("{}={}", key.name()?, value));
            Ok(magnus::r_hash::ForEach::Continue)
        }).ok();
        
        let cookie_string = cookie_pairs.join("; ");
        new_client.headers.insert("Cookie".to_string(), cookie_string);
        new_client
    }

    fn basic_auth(&self, auth_hash: RHash) -> Result<Self, MagnusError> {
        let user_key = Symbol::new("user").into_value();
        let pass_key = Symbol::new("pass").into_value();
        
        let user = auth_hash.get(user_key)
            .ok_or_else(|| MagnusError::new(exception::arg_error(), "basic_auth requires :user"))?;
        let pass = auth_hash.get(pass_key)
            .ok_or_else(|| MagnusError::new(exception::arg_error(), "basic_auth requires :pass"))?;
        
        let user_str = String::try_convert(user)?;
        let pass_str = String::try_convert(pass)?;
        
        let credentials = format!("{}:{}", user_str, pass_str);
        let encoded = magnus::eval::<String>(&format!("require 'base64'; Base64.strict_encode64('{}')", credentials))
            .map_err(|e| MagnusError::new(exception::runtime_error(), format!("Base64 encoding failed: {}", e)))?;
        
        let mut new_client = self.clone();
        new_client.headers.insert("Authorization".to_string(), format!("Basic {}", encoded));
        Ok(new_client)
    }

    fn auth(&self, auth_value: String) -> Self {
        let mut new_client = self.clone();
        new_client.headers.insert("Authorization".to_string(), auth_value);
        new_client
    }

    fn accept(&self, accept_value: Value) -> Result<Self, MagnusError> {
        let mut new_client = self.clone();
        
        let accept_header = if let Some(sym) = Symbol::from_value(accept_value) {
            let name = sym.name()?;
            let name_str: &str = &name;
            match name_str {
                "json" => "application/json",
                "xml" => "application/xml",
                "html" => "text/html",
                "text" => "text/plain",
                _ => return Err(MagnusError::new(exception::arg_error(), format!("Unknown accept type: {}", name_str))),
            }
        } else {
            &String::try_convert(accept_value)?
        };
        
        new_client.headers.insert("Accept".to_string(), accept_header.to_string());
        Ok(new_client)
    }

    fn encoding(&self, enc: String) -> Self {
        let mut new_client = self.clone();
        new_client.encoding = Some(enc);
        new_client
    }

    fn get(&self, args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
        self.ensure_open()?;
        let url_str = String::try_convert(args[0])?;
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

    fn post(&self, args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
        self.ensure_open()?;
        let url_str = String::try_convert(args[0])?;
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

    fn put(&self, args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
        self.ensure_open()?;
        let url_str = String::try_convert(args[0])?;
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

    fn delete(&self, args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
        self.ensure_open()?;
        let url_str = String::try_convert(args[0])?;
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

    fn head(&self, args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
        self.ensure_open()?;
        let url_str = String::try_convert(args[0])?;
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

    fn patch(&self, args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
        self.ensure_open()?;
        let url_str = String::try_convert(args[0])?;
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

    fn request(&self, args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
        self.ensure_open()?;
        let verb = Symbol::try_convert(args[0])?;
        let verb_name = verb.name()?;
        let verb_str: &str = &verb_name;
        let method = match verb_str {
            "get" => HttpMethod::Get,
            "post" => HttpMethod::Post,
            "put" => HttpMethod::Put,
            "delete" => HttpMethod::Delete,
            "head" => HttpMethod::Head,
            "patch" => HttpMethod::Patch,
            "options" => HttpMethod::Options,
            _ => return Err(MagnusError::new(exception::arg_error(), "Invalid HTTP verb")),
        };
        
        let url_str = String::try_convert(args[1])?;
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

    fn headers(&self, headers_hash: RHash) -> Self {
        let mut headers = HashMap::new();

        let _ = headers_hash.foreach(|key: Value, value: Value| {
            let key_str = if let Some(sym) = Symbol::from_value(key) {
                sym.name()?.to_string()
            } else {
                String::try_convert(key)?
            };
            let value_str = String::try_convert(value)?;
            let normalized_key = normalize_header_name(&key_str);
            headers.insert(normalized_key, value_str);
            Ok(ForEach::Continue)
        });

        self.with_headers(headers)
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
unsafe extern "C" fn response_free(_data: *mut std::ffi::c_void) {
    // TODO: Implement in next micro-step
}

unsafe extern "C" fn response_size(_data: *const std::ffi::c_void) -> usize {
    std::mem::size_of::<RbHttpResponse>()
}

#[magnus::wrap(class = "Wreq::HTTP::Response")]
struct RbHttpResponse {
    data: Arc<ResponseData>,
}

impl RbHttpResponse {
    async fn new(response: WreqResponse) -> Result<Self, MagnusError> {
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

fn rb_get(args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
    let client = RbHttpClient::new()?;
    client.get(args)
}

fn rb_desktop() -> Result<RbHttpClient, MagnusError> {
    RbHttpClient::new_desktop()
}

fn rb_mobile() -> Result<RbHttpClient, MagnusError> {
    RbHttpClient::new_mobile()
}

fn rb_post(args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
    let client = RbHttpClient::new()?;
    client.post(args)
}

fn rb_put(args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
    let client = RbHttpClient::new()?;
    client.put(args)
}

fn rb_delete(args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
    let client = RbHttpClient::new()?;
    client.delete(args)
}

fn rb_head(args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
    let client = RbHttpClient::new()?;
    client.head(args)
}

fn rb_patch(args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
    let client = RbHttpClient::new()?;
    client.patch(args)
}

fn rb_request(args: &[Value]) -> Result<RbHttpResponse, MagnusError> {
    let client = RbHttpClient::new()?;
    client.request(args)
}

fn rb_persistent(args: &[Value]) -> Result<RbHttpClient, MagnusError> {
    RbHttpClient::new()?.persistent(args)
}

fn rb_headers(headers_hash: RHash) -> Result<RbHttpClient, MagnusError> {
    let client = RbHttpClient::new()?;
    Ok(client.headers(headers_hash))
}

fn rb_follow(args: &[Value]) -> Result<RbHttpClient, MagnusError> {
    RbHttpClient::new()?.follow(args)
}

fn rb_timeout(secs: f64) -> Result<RbHttpClient, MagnusError> {
    Ok(RbHttpClient::new()?.timeout(secs))
}

fn rb_proxy(proxy: String) -> Result<RbHttpClient, MagnusError> {
    RbHttpClient::new()?.with_proxy(proxy)
}

fn rb_via(args: &[Value]) -> Result<RbHttpClient, MagnusError> {
    RbHttpClient::new()?.via(args)
}

fn rb_cookies(cookies_hash: RHash) -> Result<RbHttpClient, MagnusError> {
    Ok(RbHttpClient::new()?.cookies(cookies_hash))
}

fn rb_basic_auth(auth_hash: RHash) -> Result<RbHttpClient, MagnusError> {
    RbHttpClient::new()?.basic_auth(auth_hash)
}

fn rb_auth(auth_value: String) -> Result<RbHttpClient, MagnusError> {
    Ok(RbHttpClient::new()?.auth(auth_value))
}

fn rb_encoding(enc: String) -> Result<RbHttpClient, MagnusError> {
    Ok(RbHttpClient::new()?.encoding(enc))
}

fn rb_accept(accept_value: Value) -> Result<RbHttpClient, MagnusError> {
    RbHttpClient::new()?.accept(accept_value)
}

#[magnus::init]
fn init(ruby: &magnus::Ruby) -> Result<(), MagnusError> {
    let wreq_module = ruby.define_module("Wreq")?;
    let http_module = wreq_module.define_module("HTTP")?;

    let response_class = http_module.define_class("Response", ruby.class_object())?;
    response_class.define_method("status", method!(RbHttpResponse::status, 0))?;
    response_class.define_method("body", method!(RbHttpResponse::body, 0))?;
    response_class.define_method("to_s", method!(RbHttpResponse::to_s, 0))?;
    response_class.define_method("headers", method!(RbHttpResponse::headers, 0))?;
    response_class.define_method("content_type", method!(RbHttpResponse::content_type, 0))?;
    response_class.define_method("uri", method!(RbHttpResponse::uri, 0))?;
    response_class.define_method("code", method!(RbHttpResponse::code, 0))?;
    response_class.define_method("charset", method!(RbHttpResponse::charset, 0))?;

    let client_class = http_module.define_class("Client", ruby.class_object())?;
    client_class.define_singleton_method("new", function!(RbHttpClient::new, 0))?;
    client_class.define_singleton_method("new_desktop", function!(RbHttpClient::new_desktop, 0))?;
    client_class.define_singleton_method("new_mobile", function!(RbHttpClient::new_mobile, 0))?;
    client_class.define_method("with_headers", method!(RbHttpClient::with_headers, 1))?;
    client_class.define_method("follow", method!(RbHttpClient::follow, -1))?;
    client_class.define_method("timeout", method!(RbHttpClient::timeout, 1))?;
    client_class.define_method("with_proxy", method!(RbHttpClient::with_proxy, 1))?;
    client_class.define_method("via", method!(RbHttpClient::via, -1))?;
    client_class.define_method("cookies", method!(RbHttpClient::cookies, 1))?;
    client_class.define_method("basic_auth", method!(RbHttpClient::basic_auth, 1))?;
    client_class.define_method("auth", method!(RbHttpClient::auth, 1))?;
    client_class.define_method("accept", method!(RbHttpClient::accept, 1))?;
    client_class.define_method("encoding", method!(RbHttpClient::encoding, 1))?;
    client_class.define_method("get", method!(RbHttpClient::get, -1))?;
    client_class.define_method("post", method!(RbHttpClient::post, -1))?;
    client_class.define_method("put", method!(RbHttpClient::put, -1))?;
    client_class.define_method("delete", method!(RbHttpClient::delete, -1))?;
    client_class.define_method("head", method!(RbHttpClient::head, -1))?;
    client_class.define_method("patch", method!(RbHttpClient::patch, -1))?;
    client_class.define_method("request", method!(RbHttpClient::request, -1))?;
    client_class.define_method("headers", method!(RbHttpClient::headers, 1))?;
    client_class.define_method("persistent", method!(RbHttpClient::persistent, -1))?;
    client_class.define_method("close", method!(RbHttpClient::close, 0))?;

    http_module.define_module_function("get", function!(rb_get, -1))?;
    http_module.define_module_function("desktop", function!(rb_desktop, 0))?;
    http_module.define_module_function("mobile", function!(rb_mobile, 0))?;
    http_module.define_module_function("post", function!(rb_post, -1))?;
    http_module.define_module_function("put", function!(rb_put, -1))?;
    http_module.define_module_function("delete", function!(rb_delete, -1))?;
    http_module.define_module_function("head", function!(rb_head, -1))?;
    http_module.define_module_function("patch", function!(rb_patch, -1))?;
    http_module.define_module_function("request", function!(rb_request, -1))?;
    http_module.define_module_function("persistent", function!(rb_persistent, -1))?;
    http_module.define_module_function("headers", function!(rb_headers, 1))?;
    http_module.define_module_function("follow", function!(rb_follow, -1))?;
    http_module.define_module_function("timeout", function!(rb_timeout, 1))?;
    http_module.define_module_function("proxy", function!(rb_proxy, 1))?;
    http_module.define_module_function("via", function!(rb_via, -1))?;
    http_module.define_module_function("cookies", function!(rb_cookies, 1))?;
    http_module.define_module_function("basic_auth", function!(rb_basic_auth, 1))?;
    http_module.define_module_function("auth", function!(rb_auth, 1))?;
    http_module.define_module_function("accept", function!(rb_accept, 1))?;
    http_module.define_module_function("encoding", function!(rb_encoding, 1))?;

    Ok(())
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

    // Tests for get_random_desktop_emulation() - returns valid variant
    #[test]
    fn test_get_random_desktop_emulation_valid() {
        let emulation = get_random_desktop_emulation();
        // Check that we got one of the expected desktop variants
        matches!(
            emulation,
            WreqEmulation::Chrome134
                | WreqEmulation::Chrome128
                | WreqEmulation::Chrome101
                | WreqEmulation::Firefox135
                | WreqEmulation::Safari17_0
        );
    }

    // Tests for get_random_mobile_emulation() - returns valid variant
    #[test]
    fn test_get_random_mobile_emulation_valid() {
        let emulation = get_random_mobile_emulation();
        // Check that we got one of the expected mobile variants
        matches!(
            emulation,
            WreqEmulation::SafariIos17_4_1
                | WreqEmulation::SafariIos17_2
                | WreqEmulation::SafariIos16_5
                | WreqEmulation::FirefoxAndroid135
        );
    }

    // Tests for get_random_emulation() - returns valid variant (desktop or mobile)
    #[test]
    fn test_get_random_emulation_valid() {
        let emulation = get_random_emulation();
        // Should return either a desktop or mobile variant
        matches!(
            emulation,
            WreqEmulation::Chrome134
                | WreqEmulation::Chrome128
                | WreqEmulation::Chrome101
                | WreqEmulation::Firefox135
                | WreqEmulation::Safari17_0
                | WreqEmulation::SafariIos17_4_1
                | WreqEmulation::SafariIos17_2
                | WreqEmulation::SafariIos16_5
                | WreqEmulation::FirefoxAndroid135
        );
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

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use wreq::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

lazy_static::lazy_static! {
    static ref REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);
    static ref TOTAL_REQUEST_TIME: AtomicU64 = AtomicU64::new(0);
    static ref THREAD_POOL_METRICS: Mutex<HashMap<String, u64>> = Mutex::new(HashMap::new());
}

fn get_runtime() -> Arc<Runtime> {
    thread_local! {
        static RUNTIME: std::cell::RefCell<Option<Arc<Runtime>>> = std::cell::RefCell::new(None);
    }

    RUNTIME.with(|cell| {
        let mut runtime = cell.borrow_mut();
        if runtime.is_none() {
            *runtime = Some(Arc::new(Runtime::new().expect("Failed to create runtime")));
        }
        runtime.as_ref().unwrap().clone()
    })
}

fn make_request(client: &Client, url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let rt = get_runtime();
    let start_time = std::time::Instant::now();
    REQUEST_COUNT.fetch_add(1, Ordering::SeqCst);

    rt.block_on(async {
        let _response = client.get(url).send().await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;

    let duration = start_time.elapsed().as_micros() as u64;
    TOTAL_REQUEST_TIME.fetch_add(duration, Ordering::SeqCst);

    if let Ok(mut metrics) = THREAD_POOL_METRICS.lock() {
        let thread_id = thread::current().id();
        let count = metrics.entry(format!("{:?}", thread_id)).or_insert(0);
        *count += 1;
    }

    Ok(())
}

fn get_profiling_stats() -> HashMap<String, f64> {
    let mut stats = HashMap::new();

    let total_requests = REQUEST_COUNT.load(Ordering::SeqCst);
    let total_time = TOTAL_REQUEST_TIME.load(Ordering::SeqCst);

    if total_requests > 0 {
        stats.insert(
            "average_request_time_ms".to_string(),
            total_time as f64 / total_requests as f64 / 1000.0,
        );
        stats.insert("total_requests".to_string(), total_requests as f64);
        stats.insert("total_time_ms".to_string(), total_time as f64 / 1000.0);
    }

    if let Ok(metrics) = THREAD_POOL_METRICS.lock() {
        stats.insert("unique_threads".to_string(), metrics.len() as f64);
        let max_thread_requests = metrics.values().max().copied().unwrap_or(0);
        stats.insert(
            "max_requests_per_thread".to_string(),
            max_thread_requests as f64,
        );
    }

    stats
}

fn reset_profiling_stats() {
    REQUEST_COUNT.store(0, Ordering::SeqCst);
    TOTAL_REQUEST_TIME.store(0, Ordering::SeqCst);
    if let Ok(mut metrics) = THREAD_POOL_METRICS.lock() {
        metrics.clear();
    }
}

fn benchmark_sequential_requests(c: &mut Criterion) {
    let client = Client::new();
    let url = "https://httpbin.org/get";

    c.bench_function("sequential_requests", |b| {
        b.iter(|| {
            if let Err(e) = make_request(&client, black_box(url)) {
                eprintln!("Request failed: {}", e);
            }
        });
    });
}

fn benchmark_concurrent_requests(c: &mut Criterion) {
    let client = Client::new();
    let url = "https://httpbin.org/get";
    let num_threads = 10;

    c.bench_function("concurrent_requests", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..num_threads {
                let client = client.clone();
                let url = url.to_string();
                handles.push(thread::spawn(move || {
                    if let Err(e) = make_request(&client, &url) {
                        eprintln!("Request failed: {}", e);
                    }
                }));
            }
            for handle in handles {
                if let Err(e) = handle.join() {
                    eprintln!("Thread join failed: {:?}", e);
                }
            }
        });
    });
}

fn benchmark_thread_pool_utilization(c: &mut Criterion) {
    let client = Client::new();
    let url = "https://httpbin.org/get";
    let num_requests = 100;

    c.bench_function("thread_pool_utilization", |b| {
        b.iter(|| {
            reset_profiling_stats();
            let mut handles = vec![];
            for _ in 0..num_requests {
                let client = client.clone();
                let url = url.to_string();
                handles.push(thread::spawn(move || {
                    if let Err(e) = make_request(&client, &url) {
                        eprintln!("Request failed: {}", e);
                    }
                }));
            }
            for handle in handles {
                if let Err(e) = handle.join() {
                    eprintln!("Thread join failed: {:?}", e);
                }
            }
            let stats = get_profiling_stats();
            black_box(stats);
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(5));
    targets =
        benchmark_sequential_requests,
        benchmark_concurrent_requests,
        benchmark_thread_pool_utilization
}

criterion_main!(benches);

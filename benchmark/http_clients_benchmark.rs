use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::error::Error;
use std::time::Duration;

const URL: &str = "https://serpapi.com/robots.txt";

fn reqwest_blocking() -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::new();
    let _resp = client.get(URL).send()?;
    Ok(())
}

async fn reqwest_async() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let _resp = client.get(URL).send().await?;
    Ok(())
}

async fn wreq_async() -> Result<(), Box<dyn Error>> {
    let client = wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome134)
        .build()?;
    let _resp = client.get(URL).send().await?;
    Ok(())
}

fn bench_http_clients(c: &mut Criterion) {
    let mut group = c.benchmark_group("HTTP Client Comparison");

    group.measurement_time(Duration::from_secs(10));

    group.bench_function(BenchmarkId::new("reqwest", "blocking"), |b| {
        b.iter(|| {
            reqwest_blocking().unwrap();
        });
    });

    group.bench_function(BenchmarkId::new("reqwest", "async"), |b| {
        b.iter(async || {
            reqwest_async().await.unwrap();
        });
    });

    group.bench_function(BenchmarkId::new("wreq", "async"), |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                wreq_async().await.unwrap();
            });
        });
    });

    group.finish();
}

criterion_group!(benches, bench_http_clients);
criterion_main!(benches);

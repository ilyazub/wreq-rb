# Network Layer Profiling for wreq-rb

This guide covers packet-level and network layer profiling using dtrace (macOS) and eBPF (Linux) to identify network bottlenecks, connection issues, and latency sources.

## Overview

Network profiling helps identify:
- TCP connection establishment time
- TLS handshake overhead
- DNS resolution latency
- Packet retransmissions
- Network bandwidth utilization
- Keep-alive connection reuse
- HTTP/2 multiplexing efficiency

---

## macOS: dtrace + tcpdump

### 1. TCP Connection Tracing with dtrace

Track TCP connections opened by Ruby process:

```bash
# Monitor all TCP connections (requires sudo)
sudo dtrace -n 'syscall::connect:entry /execname == "ruby"/ {
    printf("Connect: %s -> %s:%d\n", 
           execname, 
           inet_ntoa((ipaddr_t *)copyin(arg1, 16)),
           ntohs(((struct sockaddr_in *)copyin(arg1, sizeof(struct sockaddr_in)))->sin_port)
    );
}'
```

### 2. Network System Call Latency

Measure time spent in network-related syscalls:

```bash
sudo dtrace -n '
syscall::connect:entry,
syscall::sendto:entry,
syscall::recvfrom:entry
/execname == "ruby"/ 
{
    self->ts = timestamp;
}

syscall::connect:return,
syscall::sendto:return,
syscall::recvfrom:return
/self->ts && execname == "ruby"/ 
{
    @time[probefunc] = quantize(timestamp - self->ts);
    self->ts = 0;
}

END {
    printf("\nNetwork syscall latency distribution (nanoseconds):\n");
}'
```

### 3. Packet Capture with tcpdump

Capture wreq-rb HTTP traffic:

```bash
# Capture to file for analysis
sudo tcpdump -i any -w wreq_traffic.pcap 'tcp port 443 or tcp port 80'

# Real-time view with timestamps
sudo tcpdump -i any -tttt 'tcp port 443 or tcp port 80'

# Capture only SYN packets (connection establishment)
sudo tcpdump -i any 'tcp[tcpflags] & tcp-syn != 0'
```

Analyze captured traffic:
```bash
# View with Wireshark
wireshark wreq_traffic.pcap

# Or use tcpdump
tcpdump -r wreq_traffic.pcap -nn
```

### 4. Complete Network Profile Script (macOS)

```bash
#!/bin/bash
# network_profile_macos.sh

PROJECT_DIR="${1:-/Users/illia/Projects/foss/wreq-rb}"
cd "$PROJECT_DIR"

echo "Starting network profiling..."

# Start tcpdump in background
sudo tcpdump -i any -w /tmp/wreq_network.pcap 'tcp port 443' &
TCPDUMP_PID=$!

# Give tcpdump time to start
sleep 2

# Run profiling workload
cat > /tmp/network_workload.rb << 'RUBY'
require './lib/wreq_rb'

HTTP = Wreq::HTTP

puts "Running network profiling workload..."

# Test 1: Connection establishment (10 sequential requests)
puts "\n1. Sequential requests (measure connection overhead):"
10.times do |i|
  start = Time.now
  response = HTTP.get('https://postman-echo.com/get')
  elapsed = (Time.now - start) * 1000
  puts "  Request #{i+1}: #{elapsed.round(0)}ms (status: #{response.status.to_i})"
end

# Test 2: Persistent connection (measure keep-alive efficiency)
puts "\n2. Persistent connection (measure keep-alive):"
HTTP.persistent('https://postman-echo.com') do |client|
  10.times do |i|
    start = Time.now
    response = client.get('/get')
    elapsed = (Time.now - start) * 1000
    puts "  Request #{i+1}: #{elapsed.round(0)}ms (status: #{response.status.to_i})"
  end
end

# Test 3: Concurrent requests (measure connection pooling)
puts "\n3. Concurrent requests (measure pooling):"
threads = 5.times.map do |i|
  Thread.new do
    start = Time.now
    response = HTTP.get('https://postman-echo.com/get')
    elapsed = (Time.now - start) * 1000
    puts "  Thread #{i+1}: #{elapsed.round(0)}ms (status: #{response.status.to_i})"
  end
end
threads.each(&:join)

puts "\nWorkload complete!"
RUBY

ruby /tmp/network_workload.rb

# Stop tcpdump
sleep 1
sudo kill -INT $TCPDUMP_PID
wait $TCPDUMP_PID 2>/dev/null

echo ""
echo "Network capture saved to: /tmp/wreq_network.pcap"
echo ""
echo "Analyze with:"
echo "  tcpdump -r /tmp/wreq_network.pcap -nn"
echo "  wireshark /tmp/wreq_network.pcap"
echo ""

# Analyze capture for common metrics
echo "=== Packet Analysis ==="
echo ""
echo "Total packets:"
tcpdump -r /tmp/wreq_network.pcap 2>/dev/null | wc -l

echo ""
echo "SYN packets (new connections):"
tcpdump -r /tmp/wreq_network.pcap 'tcp[tcpflags] & tcp-syn != 0 and tcp[tcpflags] & tcp-ack == 0' 2>/dev/null | wc -l

echo ""
echo "TLS handshake packets (Client Hello):"
tcpdump -r /tmp/wreq_network.pcap 'tcp[((tcp[12:1] & 0xf0) >> 2):1] = 0x16' 2>/dev/null | wc -l

echo ""
echo "Retransmissions:"
tcpdump -r /tmp/wreq_network.pcap 'tcp[tcpflags] & tcp-push != 0' 2>/dev/null | wc -l
```

Save as `/tmp/network_profile_macos.sh` and run:
```bash
chmod +x /tmp/network_profile_macos.sh
sudo /tmp/network_profile_macos.sh
```

---

## Linux: eBPF with bpftrace

### 1. TCP Connection Latency

Track TCP connection establishment time:

```bash
# tcp_connect_latency.bt
#!/usr/bin/env bpftrace

BEGIN {
    printf("Tracing TCP connect latency... Hit Ctrl-C to end.\n");
}

kprobe:tcp_v4_connect,
kprobe:tcp_v6_connect
{
    @start[tid] = nsecs;
}

kretprobe:tcp_v4_connect,
kretprobe:tcp_v6_connect
/@start[tid]/
{
    $duration_us = (nsecs - @start[tid]) / 1000;
    @connect_latency_us = hist($duration_us);
    delete(@start[tid]);
}

END {
    clear(@start);
}
```

Run:
```bash
sudo bpftrace tcp_connect_latency.bt
```

### 2. SSL/TLS Handshake Tracing

Track OpenSSL handshake timing:

```bash
#!/usr/bin/env bpftrace
# ssl_handshake.bt

BEGIN {
    printf("Tracing SSL/TLS handshakes...\n");
}

uprobe:/usr/lib/*/libssl.so*:SSL_do_handshake
{
    @handshake_start[tid] = nsecs;
    printf("SSL handshake started (PID %d)\n", pid);
}

uretprobe:/usr/lib/*/libssl.so*:SSL_do_handshake
/@handshake_start[tid]/
{
    $duration_ms = (nsecs - @handshake_start[tid]) / 1000000;
    @handshake_latency_ms = hist($duration_ms);
    printf("SSL handshake completed in %d ms\n", $duration_ms);
    delete(@handshake_start[tid]);
}
```

### 3. Network Bandwidth by Process

```bash
# Show bytes sent/received by wreq-rb
sudo bpftrace -e '
kprobe:tcp_sendmsg {
    @send_bytes[comm] = sum(arg2);
}

kprobe:tcp_recvmsg {
    @recv_bytes[comm] = sum(arg2);
}

interval:s:1 {
    print(@send_bytes);
    print(@recv_bytes);
    clear(@send_bytes);
    clear(@recv_bytes);
}'
```

### 4. TCP Retransmissions

Track packet retransmissions (indicator of network issues):

```bash
sudo bpftrace -e '
kprobe:tcp_retransmit_skb {
    @retransmits[comm] = count();
    printf("%s retransmitted packet\n", comm);
}

interval:s:5 {
    print(@retransmits);
}'
```

### 5. Complete eBPF Profile Script (Linux)

```bash
#!/bin/bash
# network_profile_linux.sh

PROJECT_DIR="${1:-$PWD}"
cd "$PROJECT_DIR"

echo "Starting eBPF network profiling..."

# Create bpftrace script
cat > /tmp/wreq_network.bt << 'BPFTRACE'
#!/usr/bin/env bpftrace

BEGIN {
    printf("Profiling network layer for wreq-rb...\n\n");
}

// Track TCP connections
kprobe:tcp_v4_connect {
    @connect_start[tid] = nsecs;
}

kretprobe:tcp_v4_connect /@connect_start[tid]/ {
    $duration_ms = (nsecs - @connect_start[tid]) / 1000000;
    @tcp_connect_ms = hist($duration_ms);
    delete(@connect_start[tid]);
}

// Track bytes sent/received
kprobe:tcp_sendmsg /comm == "ruby"/ {
    @bytes_sent = sum(arg2);
    @send_calls = count();
}

kprobe:tcp_recvmsg /comm == "ruby"/ {
    @bytes_recv = sum(arg2);
    @recv_calls = count();
}

// Track retransmissions
kprobe:tcp_retransmit_skb /comm == "ruby"/ {
    @retransmits = count();
}

END {
    printf("\n=== TCP Connection Latency ===\n");
    print(@tcp_connect_ms);
    
    printf("\n=== Network I/O ===\n");
    printf("Bytes sent: %d\n", @bytes_sent);
    printf("Bytes received: %d\n", @bytes_recv);
    printf("Send calls: %d\n", @send_calls);
    printf("Receive calls: %d\n", @recv_calls);
    
    printf("\n=== Retransmissions ===\n");
    printf("Total retransmits: %d\n", @retransmits);
    
    clear(@connect_start);
    clear(@tcp_connect_ms);
    clear(@bytes_sent);
    clear(@bytes_recv);
    clear(@send_calls);
    clear(@recv_calls);
    clear(@retransmits);
}
BPFTRACE

chmod +x /tmp/wreq_network.bt

# Run bpftrace in background
sudo bpftrace /tmp/wreq_network.bt &
BPFTRACE_PID=$!

# Give bpftrace time to attach
sleep 2

# Run workload
ruby /tmp/network_workload.rb

# Stop bpftrace
sleep 1
sudo kill -INT $BPFTRACE_PID
wait $BPFTRACE_PID 2>/dev/null

echo ""
echo "eBPF profiling complete!"
```

---

## Analyzing Network Performance

### Key Metrics to Monitor

1. **Connection Establishment Time**
   - **Target**: < 100ms for remote servers
   - **Includes**: DNS + TCP handshake + TLS handshake
   - **Optimization**: Use persistent connections

2. **TLS Handshake Time**
   - **Target**: 50-200ms (varies by server)
   - **Includes**: Certificate exchange, key agreement
   - **Optimization**: TLS session resumption (wreq handles this)

3. **Data Transfer Time**
   - **Target**: Depends on payload size and bandwidth
   - **Formula**: time = payload_size / bandwidth + latency
   - **Optimization**: Use compression, optimize payload

4. **Retransmission Rate**
   - **Target**: < 1% of packets
   - **Indicator**: Network quality issues
   - **Optimization**: Check network stability, firewall rules

5. **Connection Reuse**
   - **Target**: > 80% of requests reuse connections
   - **Indicator**: Keep-alive working properly
   - **Optimization**: Use persistent connections

### Example Analysis

```
=== Network Profile Results ===

TCP Connect Latency:
  Median: 45ms
  95th percentile: 120ms
  → Good! Most connections < 100ms

TLS Handshake:
  Median: 85ms
  95th percentile: 180ms
  → Normal for remote servers

Data Transfer (per request):
  Sent: 350 bytes (headers + body)
  Received: 1.2 KB (avg response)
  → Efficient payload sizes

Retransmissions: 0.3%
  → Excellent network quality

Connection Reuse:
  Sequential: 10 requests, 10 new connections (0% reuse)
  Persistent: 10 requests, 1 connection (90% reuse)
  → Use persistent connections for efficiency!
```

---

## Integration with wreq-rb

### Recommended Usage Patterns

```ruby
require './lib/wreq_rb'
HTTP = Wreq::HTTP

# ❌ BAD: New connection for each request
1000.times { HTTP.get('https://api.example.com/data') }
# Network profile: 1000 TCP handshakes + 1000 TLS handshakes

# ✅ GOOD: Reuse connections
HTTP.persistent('https://api.example.com') do |client|
  1000.times { client.get('/data') }
end
# Network profile: 1 TCP handshake + 1 TLS handshake + connection reuse
```

### Profiling Workflow

1. **Run network profiling script** (with workload)
2. **Analyze packet capture** (tcpdump/Wireshark)
3. **Check key metrics**:
   - Connection count
   - Handshake timing
   - Retransmission rate
4. **Optimize based on findings**:
   - Use persistent connections
   - Implement request batching
   - Check DNS caching

---

## Tools Reference

| Tool | Platform | Purpose | Overhead |
|------|----------|---------|----------|
| **dtrace** | macOS | System tracing | Low |
| **bpftrace** | Linux | eBPF tracing | Very low |
| **tcpdump** | Both | Packet capture | Low-Medium |
| **Wireshark** | Both | Packet analysis | N/A (offline) |
| **nettop** | macOS | Real-time monitoring | Low |
| **bcc-tools** | Linux | eBPF utilities | Very low |

---

## Further Reading

- [Brendan Gregg's Linux Performance](http://www.brendangregg.com/linuxperf.html)
- [eBPF Documentation](https://ebpf.io/)
- [dtrace Guide](https://dtrace.org/guide/)
- [TCP Performance](https://www.imperva.com/learn/performance/tcp-performance/)
- [TLS Handshake Deep Dive](https://tls13.xargs.org/)

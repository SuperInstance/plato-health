# plato-health

Health check system for PLATO rooms — uptime, response times, error rates.

## Overview

- **HealthStatus** — Healthy / Degraded / Unhealthy / Dead severity levels
- **HealthCheck** — individual check with configurable thresholds and timing
- **HealthMonitor** — aggregates multiple checks into overall health
- **LatencyTracker** — tracks response time percentiles (p50, p95, p99)
- **ErrorRateTracker** — rolling window error rate calculation

## Usage

```rust
use plato_health::*;

let mut monitor = HealthMonitor::new("room-42");
monitor.add_check(HealthCheck::new("api", HealthStatus::Healthy));
let status = monitor.overall_status();
```

## License

Apache-2.0

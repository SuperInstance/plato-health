# plato-health

> Health check system for PLATO rooms — uptime tracking, response times, error rates, and fleet status

## What This Does

plato-health provides a complete health checking system for PLATO rooms. It tracks individual room health, aggregates fleet-wide status, computes uptime percentages, and measures mean time between failures. Each health check captures response time, error count, and check count.

## The Key Idea

Health isn't binary. A room that responds in 50ms with 0 errors is healthy. A room responding in 2s with 5% errors is degraded but operational. A room with 60% errors is unhealthy. A room with no data is dead. plato-health maps these automatically based on error rate and response time thresholds, then rolls up to fleet-level metrics.

## Install

```bash
cargo add plato-health
```

## Quick Start

```rust
use plato_health::{check_room, check_fleet, uptime_percentage, HealthStatus};

// Check individual rooms
let hc = check_room("room-1", 0, 100, 50);  // 0 errors, 100 checks, 50ms
assert_eq!(hc.status, HealthStatus::Healthy);

// Check a fleet
let fleet = check_fleet(&[
    ("room-1", 0, 100, 50),
    ("room-2", 5, 100, 800),
    ("room-3", 60, 100, 5000),
]);
// [Healthy, Degraded, Dead]

// Compute uptime
let uptime = uptime_percentage(&fleet);
```

## API Reference

### Types

| Type | Description |
|---|---|
| `HealthStatus` | `Healthy` / `Degraded` / `Unhealthy` / `Dead`. Severity 0-3. `is_operational()` for Healthy/Degraded. |
| `HealthCheck` | Builder pattern: `new(id, status).with_response_time(ms).with_errors(n).with_message(msg)` |
| `HealthReport` | Aggregated report: `add_check()`, `overall_status()`, `mean_response_time_ms()`, `overall_error_rate()` |

### Functions

| Function | Description |
|---|---|
| `check_room(id, errors, total, response_ms)` | Auto-classify health from error rate + latency |
| `check_fleet(rooms)` | Batch check multiple rooms |
| `uptime_percentage(checks)` | % of checks that are operational (Healthy or Degraded) |
| `mean_time_between_failures(checks)` | Average seconds between Unhealthy/Dead checks |

### Health Classification

| Error Rate | Response Time | Status |
|---|---|---|
| 0% | < 500ms | Healthy |
| < 10% | < 2000ms | Degraded |
| < 50% | any | Unhealthy |
| ≥ 50% | any | Dead |
| no data | — | Dead |

## Testing

22 tests: status severity, operational checks, builder pattern, error rate calculation, room classification, fleet checks, uptime percentage, MTBF, report aggregation, serialization.

## License

Apache-2.0

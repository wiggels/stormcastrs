# stormcastrs

A blazing-fast weather station data collector written in Rust. Receives data from personal weather stations and exposes metrics in Prometheus format for monitoring, alerting, and visualization.

## Compatibility

Tested and verified with:
- **Ambient Weather WS-2000**

Other stations using similar query parameter formats (Ecowitt, etc.) may work but are untested.

## Features

- **Zero-config ingestion** - Works out of the box with compatible weather stations
- **Prometheus-native** - First-class `/metrics` endpoint for Grafana dashboards and alerts
- **Production-ready** - Health checks, structured logging, and graceful error handling
- **Tiny footprint** - Single static binary, minimal memory usage, sub-millisecond response times
- **Cross-platform** - Builds for Linux (x86_64, ARM64) and macOS (Intel, Apple Silicon)

## Quick Start

```bash
# Run with defaults (binds to 0.0.0.0:8080)
./stormcastrs

# Custom bind address
STORMCAST_BIND=127.0.0.1:9090 ./stormcastrs

# Enable debug logging
RUST_LOG=debug ./stormcastrs
```

Configure your weather station to push data to `http://<host>:8080/push/` and you're done.

## Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/push/` | GET | Receives weather data as query parameters |
| `/metrics` | GET | Prometheus-compatible metrics for scraping |
| `/health` | GET | Health check (returns `ok`) |

## Metrics

All weather metrics are exposed with the `weather_` prefix:

**Outdoor Sensors**
- `weather_temperature_fahrenheit` - Outdoor temperature
- `weather_humidity_percent` - Relative humidity
- `weather_wind_speed_mph` - Current wind speed
- `weather_wind_gust_mph` - Current wind gust
- `weather_max_daily_gust_mph` - Maximum gust today
- `weather_wind_direction_degrees` - Wind direction (0-359)
- `weather_wind_direction_avg10m_degrees` - 10-minute average direction
- `weather_uv_index` - UV index level
- `weather_solar_radiation_wm2` - Solar radiation (W/m²)

**Rainfall**
- `weather_rain_hourly_inches` - Rain in the last hour
- `weather_rain_event_inches` - Current rain event total
- `weather_rain_daily_inches` - Rain today
- `weather_rain_weekly_inches` - Rain this week
- `weather_rain_monthly_inches` - Rain this month
- `weather_rain_yearly_inches` - Rain this year

**Indoor Sensors**
- `weather_indoor_temperature_fahrenheit` - Indoor temperature
- `weather_indoor_humidity_percent` - Indoor humidity
- `weather_barometer_relative_inhg` - Relative barometric pressure
- `weather_barometer_absolute_inhg` - Absolute barometric pressure

**Battery Status**
- `weather_battery_outdoor` - Outdoor sensor battery (0=low, 1=ok)
- `weather_battery_indoor` - Indoor sensor battery (0=low, 1=ok)

## Prometheus Integration

Add to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'weather'
    static_configs:
      - targets: ['localhost:8080']
```

## Building from Source

```bash
# Debug build
cargo build

# Optimized release build (recommended)
cargo build --release

# Run tests
cargo test
```

The release binary is optimized with LTO and symbol stripping for minimal size.

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `STORMCAST_BIND` | `0.0.0.0:8080` | Address and port to bind |
| `RUST_LOG` | `info` | Log level (error, warn, info, debug, trace) |

## Architecture

Built on [ntex](https://github.com/ntex-rs/ntex), a high-performance async web framework. Uses a global Prometheus registry for thread-safe metric updates with lock-free gauge operations.

```
Weather Station → HTTP GET /push/?tempf=72.5&humidity=45 → stormcastrs → Prometheus → Grafana
```

## License

MIT License - see [LICENSE](LICENSE) for details.

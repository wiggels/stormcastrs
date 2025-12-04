//! stormcastrs - Weather Station Data Collector
//!
//! Receives weather data from weather stations via HTTP and exposes metrics
//! in Prometheus-compatible format for monitoring and alerting.
//!
//! Endpoints:
//!   - GET /push/     - Receive weather data as query parameters
//!   - GET /metrics   - Prometheus metrics endpoint
//!   - GET /health    - Health check endpoint

use tracing::{debug, error, info};
use ntex::web::{self, HttpResponse};
use once_cell::sync::Lazy;
use prometheus::{Encoder, Gauge, Registry, TextEncoder};
use serde::Deserialize;
use std::env;
use thiserror::Error;

// ============================================================================
// Configuration
// ============================================================================

/// Server configuration loaded from environment variables
struct Config {
    bind_addr: String,  // address:port to bind the server to
    log_level: String,  // RUST_LOG level (debug, info, warn, error)
}

impl Config {
    /// Load configuration from environment variables with sensible defaults
    fn from_env() -> Self {
        Self {
            bind_addr: env::var("STORMCAST_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            log_level: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Application-level errors with descriptive messages
#[derive(Debug, Error)]
enum AppError {
    #[error("failed to parse weather data: {0}")]
    ParseError(#[from] serde_urlencoded::de::Error),

    #[error("failed to serialize query params: {0}")]
    SerializeError(#[from] serde_urlencoded::ser::Error),

    #[error("failed to encode metrics: {0}")]
    MetricsEncodeError(#[from] prometheus::Error),

    #[error("failed to register metric '{name}': {source}")]
    MetricRegistrationError {
        name: &'static str,
        source: prometheus::Error,
    },

    #[error("server error: {0}")]
    ServerError(#[from] std::io::Error),
}

/// HTTP response conversion for AppError - returns appropriate status codes
impl web::error::WebResponseError for AppError {
    fn error_response(&self, _: &web::HttpRequest) -> HttpResponse {
        error!("{}", self);  // log all errors

        match self {
            AppError::ParseError(_) | AppError::SerializeError(_) => {
                HttpResponse::BadRequest().body(self.to_string())
            }
            _ => HttpResponse::InternalServerError().body(self.to_string()),
        }
    }
}

// ============================================================================
// Weather Data Model
// ============================================================================

/// Weather station data received from devices
///
/// All fields are optional to handle partial data from different station types.
/// Field names match the query parameter format from common weather stations
/// (e.g., Ambient Weather, Ecowitt).
#[derive(Debug, Deserialize, Default)]
struct WeatherData {
    // outdoor sensors
    tempf: Option<f32>,           // outdoor temperature (fahrenheit)
    humidity: Option<u8>,         // outdoor humidity (0-100%)
    windspeedmph: Option<f32>,    // current wind speed (mph)
    windgustmph: Option<f32>,     // current wind gust (mph)
    maxdailygust: Option<f32>,    // max gust today (mph)
    winddir: Option<u16>,         // wind direction (0-359 degrees)
    winddir_avg10m: Option<u16>,  // wind direction 10-min avg (degrees)
    uv: Option<u8>,               // uv index (0-15+)
    solarradiation: Option<f32>,  // solar radiation (W/m^2)

    // rainfall totals (inches)
    hourlyrainin: Option<f32>,    // rain in the last hour
    eventrainin: Option<f32>,     // rain for current event
    dailyrainin: Option<f32>,     // rain today
    weeklyrainin: Option<f32>,    // rain this week
    monthlyrainin: Option<f32>,   // rain this month
    yearlyrainin: Option<f32>,    // rain this year

    // indoor sensors
    tempinf: Option<f32>,         // indoor temperature (fahrenheit)
    humidityin: Option<u8>,       // indoor humidity (0-100%)
    baromrelin: Option<f32>,      // relative barometric pressure (inHg)
    baromabsin: Option<f32>,      // absolute barometric pressure (inHg)

    // battery status (typically 0=low, 1=ok)
    battout: Option<u8>,          // outdoor sensor battery
    battin: Option<u8>,           // indoor sensor battery

    // additional optional fields we don't track but shouldn't fail on
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

// ============================================================================
// Metrics Registry
// ============================================================================

/// Holds all prometheus metrics with their registry
struct Metrics {
    registry: Registry,

    // outdoor weather metrics
    temperature: Gauge,         // fahrenheit, 1 decimal
    humidity: Gauge,            // percentage, whole number
    wind_speed: Gauge,          // mph, 2 decimals
    wind_gust: Gauge,           // mph, 2 decimals
    max_daily_gust: Gauge,      // mph, 2 decimals
    wind_direction: Gauge,      // degrees, whole number
    wind_direction_avg: Gauge,  // degrees, whole number (10-min avg)
    uv_index: Gauge,            // index, whole number
    solar_radiation: Gauge,     // W/m^2, 2 decimals

    // rainfall metrics (all in inches, 3 decimals)
    rain_hourly: Gauge,
    rain_event: Gauge,
    rain_daily: Gauge,
    rain_weekly: Gauge,
    rain_monthly: Gauge,
    rain_yearly: Gauge,

    // indoor metrics
    temperature_indoor: Gauge,  // fahrenheit, 1 decimal
    humidity_indoor: Gauge,     // percentage, whole number
    barometer_relative: Gauge,  // inHg, 3 decimals
    barometer_absolute: Gauge,  // inHg, 3 decimals

    // battery status
    battery_outdoor: Gauge,     // 0=low, 1=ok
    battery_indoor: Gauge,      // 0=low, 1=ok
}

/// Create and register a gauge with the given registry
fn register_gauge(
    registry: &Registry,
    name: &'static str,
    help: &str,
) -> Result<Gauge, AppError> {
    let gauge = Gauge::new(name, help).map_err(|e| AppError::MetricRegistrationError {
        name,
        source: e,
    })?;
    registry
        .register(Box::new(gauge.clone()))
        .map_err(|e| AppError::MetricRegistrationError { name, source: e })?;
    Ok(gauge)
}

impl Metrics {
    /// Create and register all metrics with the prometheus registry
    fn new() -> Result<Self, AppError> {
        let registry = Registry::new();

        // outdoor weather
        let temperature = register_gauge(
            &registry,
            "weather_temperature_fahrenheit",
            "Outdoor temperature in Fahrenheit",
        )?;
        let humidity = register_gauge(
            &registry,
            "weather_humidity_percent",
            "Outdoor relative humidity percentage",
        )?;
        let wind_speed = register_gauge(
            &registry,
            "weather_wind_speed_mph",
            "Current wind speed in mph",
        )?;
        let wind_gust = register_gauge(
            &registry,
            "weather_wind_gust_mph",
            "Current wind gust speed in mph",
        )?;
        let max_daily_gust = register_gauge(
            &registry,
            "weather_max_daily_gust_mph",
            "Maximum wind gust today in mph",
        )?;
        let wind_direction = register_gauge(
            &registry,
            "weather_wind_direction_degrees",
            "Current wind direction in degrees (0-359)",
        )?;
        let wind_direction_avg = register_gauge(
            &registry,
            "weather_wind_direction_avg10m_degrees",
            "10-minute average wind direction in degrees",
        )?;
        let uv_index = register_gauge(
            &registry,
            "weather_uv_index",
            "Current UV index level",
        )?;
        let solar_radiation = register_gauge(
            &registry,
            "weather_solar_radiation_wm2",
            "Solar radiation in watts per square meter",
        )?;

        // rainfall
        let rain_hourly = register_gauge(
            &registry,
            "weather_rain_hourly_inches",
            "Rainfall in the last hour",
        )?;
        let rain_event = register_gauge(
            &registry,
            "weather_rain_event_inches",
            "Rainfall for the current rain event",
        )?;
        let rain_daily = register_gauge(
            &registry,
            "weather_rain_daily_inches",
            "Total rainfall today",
        )?;
        let rain_weekly = register_gauge(
            &registry,
            "weather_rain_weekly_inches",
            "Total rainfall this week",
        )?;
        let rain_monthly = register_gauge(
            &registry,
            "weather_rain_monthly_inches",
            "Total rainfall this month",
        )?;
        let rain_yearly = register_gauge(
            &registry,
            "weather_rain_yearly_inches",
            "Total rainfall this year",
        )?;

        // indoor
        let temperature_indoor = register_gauge(
            &registry,
            "weather_indoor_temperature_fahrenheit",
            "Indoor temperature in Fahrenheit",
        )?;
        let humidity_indoor = register_gauge(
            &registry,
            "weather_indoor_humidity_percent",
            "Indoor relative humidity percentage",
        )?;
        let barometer_relative = register_gauge(
            &registry,
            "weather_barometer_relative_inhg",
            "Relative barometric pressure in inches of mercury",
        )?;
        let barometer_absolute = register_gauge(
            &registry,
            "weather_barometer_absolute_inhg",
            "Absolute barometric pressure in inches of mercury",
        )?;

        // battery
        let battery_outdoor = register_gauge(
            &registry,
            "weather_battery_outdoor",
            "Outdoor sensor battery status (0=low, 1=ok)",
        )?;
        let battery_indoor = register_gauge(
            &registry,
            "weather_battery_indoor",
            "Indoor sensor battery status (0=low, 1=ok)",
        )?;

        Ok(Self {
            registry,
            temperature,
            humidity,
            wind_speed,
            wind_gust,
            max_daily_gust,
            wind_direction,
            wind_direction_avg,
            uv_index,
            solar_radiation,
            rain_hourly,
            rain_event,
            rain_daily,
            rain_weekly,
            rain_monthly,
            rain_yearly,
            temperature_indoor,
            humidity_indoor,
            barometer_relative,
            barometer_absolute,
            battery_outdoor,
            battery_indoor,
        })
    }

    /// Update all metrics from weather data (only updates if value is present)
    fn update(&self, data: &WeatherData) {
        // outdoor temperature - 1 decimal place for precision
        if let Some(v) = data.tempf {
            self.temperature.set(round(v, 1));
        }

        // outdoor humidity - whole number (percentage)
        if let Some(v) = data.humidity {
            self.humidity.set(f64::from(v));
        }

        // wind metrics - 2 decimal places for precision
        if let Some(v) = data.windspeedmph {
            self.wind_speed.set(round(v, 2));
        }
        if let Some(v) = data.windgustmph {
            self.wind_gust.set(round(v, 2));
        }
        if let Some(v) = data.maxdailygust {
            self.max_daily_gust.set(round(v, 2));
        }

        // wind direction - whole degrees
        if let Some(v) = data.winddir {
            self.wind_direction.set(f64::from(v));
        }
        if let Some(v) = data.winddir_avg10m {
            self.wind_direction_avg.set(f64::from(v));
        }

        // uv and solar - uv is whole, solar is 2 decimals
        if let Some(v) = data.uv {
            self.uv_index.set(f64::from(v));
        }
        if let Some(v) = data.solarradiation {
            self.solar_radiation.set(round(v, 2));
        }

        // rainfall - 3 decimal places for high precision
        if let Some(v) = data.hourlyrainin {
            self.rain_hourly.set(round(v, 3));
        }
        if let Some(v) = data.eventrainin {
            self.rain_event.set(round(v, 3));
        }
        if let Some(v) = data.dailyrainin {
            self.rain_daily.set(round(v, 3));
        }
        if let Some(v) = data.weeklyrainin {
            self.rain_weekly.set(round(v, 3));
        }
        if let Some(v) = data.monthlyrainin {
            self.rain_monthly.set(round(v, 3));
        }
        if let Some(v) = data.yearlyrainin {
            self.rain_yearly.set(round(v, 3));
        }

        // indoor temperature - 1 decimal place
        if let Some(v) = data.tempinf {
            self.temperature_indoor.set(round(v, 1));
        }

        // indoor humidity - whole number
        if let Some(v) = data.humidityin {
            self.humidity_indoor.set(f64::from(v));
        }

        // barometric pressure - 3 decimal places for precision
        if let Some(v) = data.baromrelin {
            self.barometer_relative.set(round(v, 3));
        }
        if let Some(v) = data.baromabsin {
            self.barometer_absolute.set(round(v, 3));
        }

        // battery status - 0 or 1
        if let Some(v) = data.battout {
            self.battery_outdoor.set(f64::from(v));
        }
        if let Some(v) = data.battin {
            self.battery_indoor.set(f64::from(v));
        }
    }

    /// Encode metrics to prometheus text format
    fn encode(&self) -> Result<Vec<u8>, AppError> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::with_capacity(4096);  // pre-allocate reasonable size
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(buffer)
    }
}

/// Global metrics instance - initialized once on first access
static METRICS: Lazy<Result<Metrics, String>> = Lazy::new(|| {
    Metrics::new().map_err(|e| e.to_string())
});

/// Get a reference to the global metrics, or return an error response
fn metrics() -> Result<&'static Metrics, AppError> {
    METRICS.as_ref().map_err(|e| {
        // this should never happen after successful startup, but handle it gracefully
        AppError::MetricRegistrationError {
            name: "global",
            source: prometheus::Error::Msg(e.clone()),
        }
    })
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Round a float to the specified number of decimal places
#[inline]
fn round(value: f32, decimals: u8) -> f64 {
    let factor = 10_f64.powi(i32::from(decimals));
    (f64::from(value) * factor).round() / factor
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// Receive weather data from a weather station
///
/// Expects query parameters matching the WeatherData struct fields.
/// Updates prometheus metrics and returns a success message.
async fn handle_weather_data(
    query: web::types::Query<std::collections::HashMap<String, String>>,
) -> Result<String, AppError> {
    let params = query.into_inner();
    debug!("received weather data: {:?}", params);

    // convert hashmap to url-encoded string for serde parsing
    let query_string = serde_urlencoded::to_string(&params)?;

    // parse into our weather data struct (missing fields become None)
    let data: WeatherData = serde_urlencoded::from_str(&query_string)?;
    debug!("parsed weather data: {:?}", data);

    // update all metrics
    metrics()?.update(&data);

    info!("weather data updated successfully");
    Ok("ok".to_string())
}

/// Expose prometheus metrics for scraping
async fn handle_metrics() -> Result<HttpResponse, AppError> {
    debug!("metrics endpoint called");

    let buffer = metrics()?.encode()?;

    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(buffer))
}

/// Health check endpoint for load balancers and monitoring
async fn handle_health() -> HttpResponse {
    // verify metrics are initialized correctly
    match METRICS.as_ref() {
        Ok(_) => HttpResponse::Ok().body("ok"),
        Err(e) => {
            error!("health check failed: {}", e);
            HttpResponse::ServiceUnavailable().body(format!("unhealthy: {}", e))
        }
    }
}

// ============================================================================
// Application Entry Point
// ============================================================================

#[ntex::main]
async fn main() -> Result<(), AppError> {
    // load configuration from environment
    let config = Config::from_env();

    // initialize tracing subscriber (respects RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level)),
        )
        .init();

    // eagerly initialize metrics to catch registration errors at startup
    if let Err(e) = METRICS.as_ref() {
        error!("failed to initialize metrics: {}", e);
        return Err(AppError::MetricRegistrationError {
            name: "initialization",
            source: prometheus::Error::Msg(e.clone()),
        });
    }

    info!("starting stormcastrs on {}", config.bind_addr);

    // start the web server
    web::server(|| {
        web::App::new()
            .route("/push/", web::get().to(handle_weather_data))  // weather data ingestion
            .route("/metrics", web::get().to(handle_metrics))     // prometheus scrape endpoint
            .route("/health", web::get().to(handle_health))       // health check for lb/k8s
    })
    .bind(&config.bind_addr)?
    .run()
    .await?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round() {
        assert_eq!(round(72.456, 1), 72.5);
        assert_eq!(round(72.444, 1), 72.4);
        assert_eq!(round(0.123456, 3), 0.123);
        assert_eq!(round(0.1239, 3), 0.124);
    }

    #[test]
    fn test_weather_data_partial() {
        // should parse even with missing fields
        let data: WeatherData = serde_urlencoded::from_str("tempf=72.5&humidity=45").unwrap();
        assert_eq!(data.tempf, Some(72.5));
        assert_eq!(data.humidity, Some(45));
        assert_eq!(data.windspeedmph, None);
    }

    #[test]
    fn test_weather_data_empty() {
        // should handle completely empty data
        let data: WeatherData = serde_urlencoded::from_str("").unwrap();
        assert_eq!(data.tempf, None);
    }

    #[test]
    fn test_config_defaults() {
        // clear env vars for test
        env::remove_var("STORMCAST_BIND");
        let config = Config::from_env();
        assert_eq!(config.bind_addr, "0.0.0.0:8080");
    }
}

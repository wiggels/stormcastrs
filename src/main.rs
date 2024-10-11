use ntex::web;
use prometheus::{Encoder, TextEncoder, register_gauge, Gauge};
use serde::Deserialize;
use lazy_static::lazy_static;
use std::collections::HashMap;
use log::info; // For logging

#[derive(Debug, Deserialize)]
struct WeatherData {
    tempf: f32,
    humidity: u8,
    windspeedmph: f32,
    windgustmph: f32,
    maxdailygust: f32,
    winddir: u16,
    winddir_avg10m: u16,
    uv: u8,
    solarradiation: f32,
    hourlyrainin: f32,
    eventrainin: f32,
    dailyrainin: f32,
    weeklyrainin: f32,
    monthlyrainin: f32,
    yearlyrainin: f32,
    battout: u8,
    tempinf: f32,
    humidityin: u8,
    baromrelin: f32,
    baromabsin: f32,
    battin: u8,
}

// Define Prometheus metrics
lazy_static! {
    static ref TEMP_GAUGE: Gauge = register_gauge!(
        "weather_temperature_fahrenheit",
        "Outdoor temperature in Fahrenheit"
    )
    .unwrap();
    static ref HUMIDITY_GAUGE: Gauge = register_gauge!(
        "weather_humidity_percentage",
        "Outdoor humidity percentage"
    )
    .unwrap();
    static ref WIND_SPEED_GAUGE: Gauge = register_gauge!(
        "weather_windspeed_mph",
        "Windspeed in miles per hour"
    )
    .unwrap();
    static ref WIND_GUST_GAUGE: Gauge = register_gauge!(
        "weather_windgust_mph",
        "Wind gust in miles per hour"
    )
    .unwrap();
    static ref MAX_DAILY_GUST_GAUGE: Gauge = register_gauge!(
        "weather_max_daily_gust_mph",
        "Maximum daily wind gust in miles per hour"
    )
    .unwrap();
    static ref WIND_DIR_GAUGE: Gauge = register_gauge!(
        "weather_wind_direction_degrees",
        "Wind direction in degrees"
    )
    .unwrap();
    static ref WIND_DIR_AVG10M_GAUGE: Gauge = register_gauge!(
        "weather_wind_direction_avg10m_degrees",
        "Wind direction averaged over 10 minutes in degrees"
    )
    .unwrap();
    static ref UV_INDEX_GAUGE: Gauge = register_gauge!(
        "weather_uv_index",
        "UV index level"
    )
    .unwrap();
    static ref SOLAR_RADIATION_GAUGE: Gauge = register_gauge!(
        "weather_solar_radiation",
        "Solar radiation level"
    )
    .unwrap();
    static ref HOURLY_RAIN_GAUGE: Gauge = register_gauge!(
        "weather_hourly_rain_in",
        "Rainfall in the last hour in inches"
    )
    .unwrap();
    static ref EVENT_RAIN_GAUGE: Gauge = register_gauge!(
        "weather_event_rain_in",
        "Rainfall for a specific event in inches"
    )
    .unwrap();
    static ref DAILY_RAIN_GAUGE: Gauge = register_gauge!(
        "weather_daily_rain_in",
        "Daily rainfall in inches"
    )
    .unwrap();
    static ref WEEKLY_RAIN_GAUGE: Gauge = register_gauge!(
        "weather_weekly_rain_in",
        "Weekly rainfall in inches"
    )
    .unwrap();
    static ref MONTHLY_RAIN_GAUGE: Gauge = register_gauge!(
        "weather_monthly_rain_in",
        "Monthly rainfall in inches"
    )
    .unwrap();
    static ref YEARLY_RAIN_GAUGE: Gauge = register_gauge!(
        "weather_yearly_rain_in",
        "Yearly rainfall in inches"
    )
    .unwrap();
    static ref BATT_OUT_GAUGE: Gauge = register_gauge!(
        "weather_battout_level",
        "Outdoor battery level"
    )
    .unwrap();
    static ref TEMP_INDOOR_GAUGE: Gauge = register_gauge!(
        "weather_indoor_temperature_fahrenheit",
        "Indoor temperature in Fahrenheit"
    )
    .unwrap();
    static ref HUMIDITY_INDOOR_GAUGE: Gauge = register_gauge!(
        "weather_indoor_humidity_percentage",
        "Indoor humidity percentage"
    )
    .unwrap();
    static ref BAROM_REL_GAUGE: Gauge = register_gauge!(
        "weather_barom_relative_in",
        "Relative barometric pressure in inches"
    )
    .unwrap();
    static ref BAROM_ABS_GAUGE: Gauge = register_gauge!(
        "weather_barom_absolute_in",
        "Absolute barometric pressure in inches"
    )
    .unwrap();
    static ref BATT_IN_GAUGE: Gauge = register_gauge!(
        "weather_battin_level",
        "Indoor battery level"
    )
    .unwrap();
}

fn round_to_places(value: f32, places: i32) -> f64 {
    let factor = 10f32.powi(places);
    (value * factor).round() as f64 / factor as f64
}

fn set_round_gauge(gauge: &Gauge, value: f32, places: i32) {
    gauge.set(round_to_places(value, places));
}

async fn handle_weather_data(
    query: web::types::Query<HashMap<String, String>>,
) -> String {
    // Extract the underlying HashMap from the Query
    let query_params = query.into_inner();

    // Log that we received data
    info!("Received data: {:?}", query_params);

    // Serialize the query parameters into a URL-encoded string
    let query_string = serde_urlencoded::to_string(query_params).unwrap();

    // Deserialize the query parameters into WeatherData
    let weather_data: WeatherData = match serde_urlencoded::from_str(&query_string) {
        Ok(data) => data,
        Err(e) => {
            info!("Error parsing query params: {}", e);
            return format!("Error parsing query params: {}", e);
        }
    };

    // Log the weather data
    info!("Parsed weather data: {:?}", weather_data);

    // Update Prometheus metrics with appropriate decimal places
    set_round_gauge(&TEMP_GAUGE, weather_data.tempf, 1);               // Temperature (outdoor) with 1 decimal place
    HUMIDITY_GAUGE.set(weather_data.humidity as f64);                  // Humidity (outdoor) no decimal places
    set_round_gauge(&WIND_SPEED_GAUGE, weather_data.windspeedmph, 2);  // Wind speed with 2 decimal places
    set_round_gauge(&WIND_GUST_GAUGE, weather_data.windgustmph, 2);    // Wind gust with 2 decimal places
    set_round_gauge(&MAX_DAILY_GUST_GAUGE, weather_data.maxdailygust, 2); // Max daily gust with 2 decimal places
    WIND_DIR_GAUGE.set(weather_data.winddir as f64);                   // Wind direction with no decimal places
    WIND_DIR_AVG10M_GAUGE.set(weather_data.winddir_avg10m as f64);     // Wind direction (10m average) no decimal places
    UV_INDEX_GAUGE.set(weather_data.uv as f64);                        // UV index no decimal places
    set_round_gauge(&SOLAR_RADIATION_GAUGE, weather_data.solarradiation, 2); // Solar radiation with 2 decimal places

    // Set rain-related metrics (3 decimal places)
    set_round_gauge(&HOURLY_RAIN_GAUGE, weather_data.hourlyrainin, 3); // Hourly rain with 3 decimal places
    set_round_gauge(&EVENT_RAIN_GAUGE, weather_data.eventrainin, 3);   // Event rain with 3 decimal places
    set_round_gauge(&DAILY_RAIN_GAUGE, weather_data.dailyrainin, 3);   // Daily rain with 3 decimal places
    set_round_gauge(&WEEKLY_RAIN_GAUGE, weather_data.weeklyrainin, 3); // Weekly rain with 3 decimal places
    set_round_gauge(&MONTHLY_RAIN_GAUGE, weather_data.monthlyrainin, 3); // Monthly rain with 3 decimal places
    set_round_gauge(&YEARLY_RAIN_GAUGE, weather_data.yearlyrainin, 3); // Yearly rain with 3 decimal places

    BATT_OUT_GAUGE.set(weather_data.battout as f64);                   // Battery (outdoor) no decimal places
    set_round_gauge(&TEMP_INDOOR_GAUGE, weather_data.tempinf, 1);      // Temperature (indoor) with 1 decimal place
    HUMIDITY_INDOOR_GAUGE.set(weather_data.humidityin as f64);         // Humidity (indoor) no decimal places
    set_round_gauge(&BAROM_REL_GAUGE, weather_data.baromrelin, 3);     // Relative barometric pressure with 3 decimal places
    set_round_gauge(&BAROM_ABS_GAUGE, weather_data.baromabsin, 3);     // Absolute barometric pressure with 3 decimal places
    BATT_IN_GAUGE.set(weather_data.battin as f64);                     // Battery (indoor) no decimal places

    // Respond with success
    "Data received and metrics updated".to_string()
}


async fn handle_metrics() -> web::HttpResponse {
    info!("Called metrics endpoint: {}", 1);
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();

    // Encode metrics into text format that Prometheus understands
    encoder.encode(&metric_families, &mut buffer).unwrap();

    web::HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(buffer)
}

#[ntex::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::init();

    // Start the web server
    web::server(|| {
        web::App::new()
            .route("/push/", web::get().to(handle_weather_data)) // Receive weather data
            .route("/metrics", web::get().to(handle_metrics))    // Expose metrics for Prometheus
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}

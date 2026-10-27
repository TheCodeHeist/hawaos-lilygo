use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::models::{SharedState, WeatherForecastDayMsg, WeatherMsg};

pub fn map_wmo_code(code: u64, is_day: bool) -> (&'static str, &'static str) {
    match code {
        0 => {
            if is_day {
                ("Clear", "sun")
            } else {
                ("Clear", "moon")
            }
        }
        1 => {
            if is_day {
                ("Mainly Clear", "cloud-sun")
            } else {
                ("Mainly Clear", "cloud-moon")
            }
        }
        2 => {
            if is_day {
                ("Partly Cloudy", "cloud-sun")
            } else {
                ("Partly Cloudy", "cloud-moon")
            }
        }
        3 => ("Overcast", "cloud"),
        45 | 48 => ("Foggy", "cloud-fog"),
        51 | 53 | 55 => ("Drizzle", "cloud-rain"),
        56 | 57 => ("Freezing Drizzle", "cloud-rain"),
        61 | 63 | 65 => ("Rain", "cloud-rain"),
        66 | 67 => ("Freezing Rain", "cloud-rain"),
        71 | 73 | 75 => ("Snow", "cloud-snow"),
        77 => ("Snow Grains", "cloud-snow"),
        80 | 81 | 82 => ("Rain Showers", "cloud-rain"),
        85 | 86 => ("Snow Showers", "cloud-snow"),
        95 => ("Thunderstorm", "cloud-lightning"),
        96 | 99 => ("Thunderstorm w/ Hail", "cloud-lightning"),
        _ => ("Cloudy", "cloud"),
    }
}

pub fn day_of_week_str(date_str: &str) -> &'static str {
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[0].parse::<i32>(),
            parts[1].parse::<i32>(),
            parts[2].parse::<i32>(),
        ) {
            let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
            let y_adj = if m < 3 { y - 1 } else { y };
            let dow = (y_adj + y_adj / 4 - y_adj / 100 + y_adj / 400 + t[(m - 1) as usize] + d) % 7;
            return match dow {
                0 => "Sun",
                1 => "Mon",
                2 => "Tue",
                3 => "Wed",
                4 => "Thu",
                5 => "Fri",
                6 => "Sat",
                _ => "---",
            };
        }
    }
    "---"
}

pub fn fetch_open_meteo_weather(
    auto_location: bool,
    custom_city: &str,
    unit: &str,
) -> Option<WeatherMsg> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(5000)))
        .build();
    let agent: ureq::Agent = config.into();

    let mut lat = 23.77;
    let mut lon = 90.38;
    let mut resolved_city = String::from("Dhaka");

    let is_custom = !auto_location && !custom_city.trim().is_empty();

    if is_custom {
        let city_query = custom_city.trim();
        let geocode_url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
            city_query.replace(' ', "%20")
        );
        if let Ok(mut resp) = agent.get(&geocode_url).call() {
            if let Ok(bytes) = resp.body_mut().read_to_vec() {
                if let Ok(geo_json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if let Some(res) = geo_json["results"].as_array().and_then(|a| a.first()) {
                        if let (Some(g_lat), Some(g_lon)) = (res["latitude"].as_f64(), res["longitude"].as_f64()) {
                            lat = g_lat;
                            lon = g_lon;
                            if let Some(name) = res["name"].as_str() {
                                resolved_city = name.to_string();
                            }
                        }
                    }
                }
            }
        }
    } else {
        // IP Geolocation via ip-api.com
        if let Ok(mut resp) = agent.get("http://ip-api.com/json/").call() {
            if let Ok(bytes) = resp.body_mut().read_to_vec() {
                if let Ok(ip_json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if ip_json["status"].as_str() == Some("success") {
                        if let (Some(ip_lat), Some(ip_lon)) = (ip_json["lat"].as_f64(), ip_json["lon"].as_f64()) {
                            lat = ip_lat;
                            lon = ip_lon;
                        }
                        if let Some(city) = ip_json["city"].as_str() {
                            resolved_city = city.to_string();
                        }
                    }
                }
            }
        }
    }

    let unit_str = if unit.eq_ignore_ascii_case("f") { "f" } else { "c" };
    let unit_params = if unit_str == "f" {
        "&temperature_unit=fahrenheit&wind_speed_unit=mph"
    } else {
        ""
    };

    let forecast_url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,is_day,weather_code,wind_speed_10m&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=3{}",
        lat, lon, unit_params
    );

    let mut resp = agent.get(&forecast_url).call().ok()?;
    let bytes = resp.body_mut().read_to_vec().ok()?;
    let fc_json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;

    let current = &fc_json["current"];
    let daily = &fc_json["daily"];

    let temp = current["temperature_2m"].as_f64().unwrap_or(0.0).round() as i16;
    let feels_like = current["apparent_temperature"].as_f64().unwrap_or(temp as f64).round() as i16;
    let humidity = current["relative_humidity_2m"].as_f64().unwrap_or(0.0).round().clamp(0.0, 100.0) as u8;
    let wind_speed = current["wind_speed_10m"].as_f64().unwrap_or(0.0).round() as u16;
    let is_day = current["is_day"].as_u64().unwrap_or(1) == 1;
    let weather_code = current["weather_code"].as_u64().unwrap_or(0);
    let (condition, icon) = map_wmo_code(weather_code, is_day);

    let mut forecast_days = Vec::new();
    let times = daily["time"].as_array();
    let codes = daily["weather_code"].as_array();
    let maxs = daily["temperature_2m_max"].as_array();
    let mins = daily["temperature_2m_min"].as_array();

    let mut temp_min = temp;
    let mut temp_max = temp;

    for i in 0..3 {
        let day_label = if i == 0 {
            "Today"
        } else if i == 1 {
            "Tmrw"
        } else if let Some(t) = times.and_then(|a| a.get(i)).and_then(|v| v.as_str()) {
            day_of_week_str(t)
        } else {
            "---"
        };

        let day_code = codes.and_then(|a| a.get(i)).and_then(|v| v.as_u64()).unwrap_or(0);
        let (_, day_icon) = map_wmo_code(day_code, true);

        let d_min = mins.and_then(|a| a.get(i)).and_then(|v| v.as_f64()).unwrap_or(temp as f64).round() as i16;
        let d_max = maxs.and_then(|a| a.get(i)).and_then(|v| v.as_f64()).unwrap_or(temp as f64).round() as i16;

        if i == 0 {
            temp_min = d_min;
            temp_max = d_max;
        }

        forecast_days.push(WeatherForecastDayMsg {
            day: day_label.to_string(),
            icon: day_icon.to_string(),
            min: d_min,
            max: d_max,
        });
    }

    Some(WeatherMsg {
        msg_type: "weather".to_string(),
        city: resolved_city.trim().to_string(),
        temp,
        feels_like,
        temp_min,
        temp_max,
        humidity,
        wind_speed,
        unit: unit_str.to_string(),
        condition: condition.to_string(),
        icon: icon.to_string(),
        forecast: forecast_days,
    })
}

pub fn trigger_weather_update(state: &SharedState) {
    let state_clone = Arc::clone(state);
    thread::spawn(move || {
        let (auto_loc, city, unit) = if let Ok(g) = state_clone.lock() {
            (
                g.config.weather_auto_location,
                g.config.weather_city.clone(),
                g.config.weather_unit.clone(),
            )
        } else {
            return;
        };

        if let Some(w) = fetch_open_meteo_weather(auto_loc, &city, &unit) {
            if let Ok(mut g) = state_clone.lock() {
                g.push_log(format!(
                    "Weather RX: {} {}°{} ({})",
                    w.city,
                    w.temp,
                    w.unit.to_uppercase(),
                    w.condition
                ));
                g.weather = Some(w);
            }
        } else {
            if let Ok(mut g) = state_clone.lock() {
                g.push_log("Weather fetch failed (offline or timeout)");
            }
        }
    });
}

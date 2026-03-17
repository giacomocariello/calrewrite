use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::{Duration, NaiveDateTime};
use clap::Parser;
use regex::Regex;
use std::net::SocketAddr;

/// iCal timeshift proxy — fetches an ICS feed and shifts all datetimes
/// by a configurable offset.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Address to bind to
    #[arg(long, default_value = "0.0.0.0", env = "CALREWRITE_HOST")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 3000, env = "CALREWRITE_PORT")]
    port: u16,
}

#[derive(serde::Deserialize)]
struct Params {
    url: String,
    shift: i64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .expect("invalid host:port");

    let app = Router::new().route("/", get(handler));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    eprintln!("listening on {addr}");
    axum::serve(listener, app).await.expect("server error");
}

async fn handler(Query(params): Query<Params>) -> impl IntoResponse {
    let body = match fetch_ical(&params.url).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                HeaderMap::new(),
                format!("failed to fetch upstream ical: {e}"),
            );
        }
    };

    let shifted = shift_ical(&body, params.shift);

    let mut headers = HeaderMap::new();
    headers.insert("content-type", "text/calendar; charset=utf-8".parse().unwrap());

    (StatusCode::OK, headers, shifted)
}

async fn fetch_ical(url: &str) -> Result<String, reqwest::Error> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;
    let resp = client.get(url).send().await?.error_for_status()?;
    resp.text().await
}

/// Shift all iCal datetime properties by `shift_secs` seconds.
///
/// Handles both UTC timestamps (YYYYMMDDTHHMMSSZ) and bare
/// date-times (YYYYMMDDTHHMMSS). Date-only values (YYYYMMDD)
/// are left untouched since a sub-day shift on an all-day event
/// is ambiguous.
fn shift_ical(input: &str, shift_secs: i64) -> String {
    // Matches the VALUE portion of datetime properties like DTSTART, DTEND,
    // DTSTART;TZID=..., DTSTAMP, CREATED, LAST-MODIFIED, etc.
    // Captures: full datetime with optional Z suffix.
    let re = Regex::new(
        r"(?m)^((?:DTSTART|DTEND|DTSTAMP|CREATED|LAST-MODIFIED|RECURRENCE-ID|EXDATE|RDATE|DUE|COMPLETED|TRIGGER)(?:;[^:]*)?:)(\d{8}T\d{6})(Z?)\r?\n",
    )
    .expect("invalid regex");

    let duration = Duration::seconds(shift_secs);

    re.replace_all(input, |caps: &regex::Captures| {
        let prefix = &caps[1];
        let dt_str = &caps[2];
        let z_suffix = &caps[3];

        let shifted = match NaiveDateTime::parse_from_str(dt_str, "%Y%m%dT%H%M%S") {
            Ok(dt) => {
                let new_dt = dt + duration;
                format!("{prefix}{}{z_suffix}\r\n", new_dt.format("%Y%m%dT%H%M%S"))
            }
            Err(_) => caps[0].to_string(),
        };
        shifted
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_forward() {
        let ical = "BEGIN:VEVENT\r\nDTSTART:20250101T120000Z\r\nDTEND:20250101T130000Z\r\nSUMMARY:Test\r\nEND:VEVENT\r\n";
        let result = shift_ical(ical, 3600);
        assert!(result.contains("DTSTART:20250101T130000Z"));
        assert!(result.contains("DTEND:20250101T140000Z"));
        assert!(result.contains("SUMMARY:Test"));
    }

    #[test]
    fn shift_backward() {
        let ical = "BEGIN:VEVENT\r\nDTSTART:20250101T120000Z\r\nDTEND:20250101T130000Z\r\nEND:VEVENT\r\n";
        let result = shift_ical(ical, -7200);
        assert!(result.contains("DTSTART:20250101T100000Z"));
        assert!(result.contains("DTEND:20250101T110000Z"));
    }

    #[test]
    fn shift_with_tzid() {
        let ical = "BEGIN:VEVENT\r\nDTSTART;TZID=Europe/Berlin:20250615T090000\r\nDTEND;TZID=Europe/Berlin:20250615T100000\r\nEND:VEVENT\r\n";
        let result = shift_ical(ical, 1800);
        assert!(result.contains("DTSTART;TZID=Europe/Berlin:20250615T093000"));
        assert!(result.contains("DTEND;TZID=Europe/Berlin:20250615T103000"));
    }

    #[test]
    fn no_shift() {
        let ical = "BEGIN:VEVENT\r\nDTSTART:20250101T120000Z\r\nSUMMARY:Hello\r\nEND:VEVENT\r\n";
        let result = shift_ical(ical, 0);
        assert!(result.contains("DTSTART:20250101T120000Z"));
    }

    #[test]
    fn preserves_non_datetime_lines() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nDTSTART:20250101T120000Z\r\nSUMMARY:Keep me\r\nLOCATION:Somewhere\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let result = shift_ical(ical, 60);
        assert!(result.contains("VERSION:2.0"));
        assert!(result.contains("SUMMARY:Keep me"));
        assert!(result.contains("LOCATION:Somewhere"));
    }
}

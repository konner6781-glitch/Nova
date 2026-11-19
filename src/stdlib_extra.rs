use std::fs;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256, Sha512};

use crate::value::Value;

static RNG_STATE: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);

pub fn call(module: &str, name: &str, args: &[Value]) -> Result<Value, String> {
    match module {
        "Random" => random(name, args),
        "Crypto" => crypto(name, args),
        "CSV" => csv(name, args),
        "Logging" => logging(name, args),
        "DateTime" => datetime(name, args),
        "Process" | "Program" => process_api(module, name, args),
        "HTTP" => http(name, args),
        "Network" => network(name, args),
        _ => Err(format!("Unknown standard library module '{}'.", module)),
    }
}

fn arity(label: &str, args: &[Value], n: usize) -> Result<(), String> {
    if args.len() == n {
        Ok(())
    } else {
        Err(format!(
            "{} expects {} argument(s), received {}.",
            label,
            n,
            args.len()
        ))
    }
}
fn string<'a>(label: &str, v: &'a Value) -> Result<&'a str, String> {
    if let Value::String(s) = v {
        Ok(s)
    } else {
        Err(format!(
            "{} expects String, received {}.",
            label,
            v.type_name()
        ))
    }
}
fn int(label: &str, v: &Value) -> Result<i64, String> {
    if let Value::Int(n) = v {
        Ok(*n)
    } else {
        Err(format!(
            "{} expects Int, received {}.",
            label,
            v.type_name()
        ))
    }
}
fn next_random() -> u64 {
    let mut old = RNG_STATE.load(Ordering::Relaxed);
    loop {
        if old == 0 {
            old = 0x9e37_79b9_7f4a_7c15;
        }
        let mut next = old;
        next ^= next << 13;
        next ^= next >> 7;
        next ^= next << 17;
        match RNG_STATE.compare_exchange_weak(old, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(actual) => old = actual,
        }
    }
}
fn random(name: &str, a: &[Value]) -> Result<Value, String> {
    match name {
        "seed" => {
            arity("Random.seed", a, 1)?;
            RNG_STATE.store(int("Random.seed", &a[0])? as u64, Ordering::Relaxed);
            Ok(Value::None)
        }
        "integer" => {
            arity("Random.integer", a, 2)?;
            let lo = int("Random.integer", &a[0])?;
            let hi = int("Random.integer", &a[1])?;
            if lo > hi {
                return Err("Random.integer minimum must not exceed maximum.".into());
            }
            let span = (hi as i128 - lo as i128 + 1) as u128;
            Ok(Value::Int(
                (lo as i128 + (next_random() as u128 % span) as i128) as i64,
            ))
        }
        "float" => {
            arity("Random.float", a, 0)?;
            Ok(Value::Float(
                (next_random() >> 11) as f64 / ((1u64 << 53) as f64),
            ))
        }
        "boolean" => {
            arity("Random.boolean", a, 0)?;
            Ok(Value::Bool(next_random() & 1 == 1))
        }
        "choice" => {
            arity("Random.choice", a, 1)?;
            let Value::List(xs) = &a[0] else {
                return Err("Random.choice expects a List.".into());
            };
            if xs.is_empty() {
                return Err("Random.choice cannot select from an empty List.".into());
            }
            Ok(xs[(next_random() as usize) % xs.len()].clone())
        }
        "shuffle" => {
            arity("Random.shuffle", a, 1)?;
            let Value::List(xs) = &a[0] else {
                return Err("Random.shuffle expects a List.".into());
            };
            let mut xs = xs.clone();
            for i in (1..xs.len()).rev() {
                xs.swap(i, (next_random() as usize) % (i + 1));
            }
            Ok(Value::List(xs))
        }
        _ => Err(format!("Random has no function '{}'.", name)),
    }
}

fn crypto(name: &str, a: &[Value]) -> Result<Value, String> {
    match name {
        "sha256" | "sha512" => {
            arity(&format!("Crypto.{}", name), a, 1)?;
            let s = string("Crypto", &a[0])?;
            let out = if name == "sha256" {
                Sha256::digest(s.as_bytes()).to_vec()
            } else {
                Sha512::digest(s.as_bytes()).to_vec()
            };
            Ok(Value::String(
                out.iter().map(|b| format!("{:02x}", b)).collect(),
            ))
        }
        "hmac_sha256" => {
            arity("Crypto.hmac_sha256", a, 2)?;
            let key = string("Crypto.hmac_sha256", &a[0])?.as_bytes();
            let data = string("Crypto.hmac_sha256", &a[1])?.as_bytes();
            let mut block = [0u8; 64];
            if key.len() > 64 {
                block[..32].copy_from_slice(&Sha256::digest(key));
            } else {
                block[..key.len()].copy_from_slice(key);
            }
            let mut inner = Vec::with_capacity(64 + data.len());
            inner.extend(block.iter().map(|b| b ^ 0x36));
            inner.extend_from_slice(data);
            let inner_hash = Sha256::digest(inner);
            let mut outer = Vec::with_capacity(96);
            outer.extend(block.iter().map(|b| b ^ 0x5c));
            outer.extend_from_slice(&inner_hash);
            let digest = Sha256::digest(outer);
            Ok(Value::String(
                digest.iter().map(|b| format!("{:02x}", b)).collect(),
            ))
        }
        "random_bytes" => {
            arity("Crypto.random_bytes", a, 1)?;
            let n = int("Crypto.random_bytes", &a[0])?;
            if !(0..=1_048_576).contains(&n) {
                return Err("Crypto.random_bytes size must be between 0 and 1048576.".into());
            }
            let mut bytes = vec![0u8; n as usize];
            secure_random(&mut bytes)?;
            Ok(Value::List(
                bytes.into_iter().map(|b| Value::Int(b as i64)).collect(),
            ))
        }
        _ => Err(format!("Crypto has no function '{}'.", name)),
    }
}
#[cfg(unix)]
fn secure_random(out: &mut [u8]) -> Result<(), String> {
    fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(out))
        .map_err(|e| format!("Operating system secure random source failed: {}", e))
}
#[cfg(windows)]
fn secure_random(out: &mut [u8]) -> Result<(), String> {
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(
            algorithm: *mut core::ffi::c_void,
            buffer: *mut u8,
            length: u32,
            flags: u32,
        ) -> i32;
    }
    let len = u32::try_from(out.len()).map_err(|_| "Random request too large.")?;
    let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), out.as_mut_ptr(), len, 2) };
    if status < 0 {
        Err(format!(
            "Operating system secure random source failed with status {}.",
            status
        ))
    } else {
        Ok(())
    }
}
#[cfg(not(any(unix, windows)))]
fn secure_random(_: &mut [u8]) -> Result<(), String> {
    Err("Secure random bytes are unsupported on this operating system.".into())
}

fn csv(name: &str, a: &[Value]) -> Result<Value, String> {
    match name {
        "parse" | "read" => {
            let text = if name == "parse" {
                arity("CSV.parse", a, 1)?;
                string("CSV.parse", &a[0])?.to_owned()
            } else {
                arity("CSV.read", a, 1)?;
                let p = string("CSV.read", &a[0])?;
                fs::read_to_string(p).map_err(|e| format!("Cannot read CSV '{}': {}", p, e))?
            };
            Ok(Value::List(
                parse_csv(&text)?
                    .into_iter()
                    .map(|r| Value::List(r.into_iter().map(Value::String).collect()))
                    .collect(),
            ))
        }
        "stringify" | "write" => {
            let (rows, path) = if name == "stringify" {
                arity("CSV.stringify", a, 1)?;
                (&a[0], None)
            } else {
                arity("CSV.write", a, 2)?;
                (&a[1], Some(string("CSV.write", &a[0])?))
            };
            let Value::List(rows) = rows else {
                return Err("CSV expects a List of rows.".into());
            };
            let mut out = String::new();
            for (ri, row) in rows.iter().enumerate() {
                let Value::List(cells) = row else {
                    return Err(format!("CSV row {} is not a List.", ri));
                };
                for (ci, cell) in cells.iter().enumerate() {
                    if ci > 0 {
                        out.push(',')
                    }
                    let s = cell.to_string_value();
                    if s.contains([',', '"', '\n', '\r']) {
                        out.push('"');
                        for c in s.chars() {
                            if c == '"' {
                                out.push('"')
                            }
                            out.push(c)
                        }
                        out.push('"')
                    } else {
                        out.push_str(&s)
                    }
                }
                out.push_str("\r\n")
            }
            if let Some(p) = path {
                fs::write(p, &out).map_err(|e| format!("Cannot write CSV '{}': {}", p, e))?;
                Ok(Value::None)
            } else {
                Ok(Value::String(out))
            }
        }
        _ => Err(format!("CSV has no function '{}'.", name)),
    }
}
fn parse_csv(s: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let (mut row, mut field, mut chars, mut quoted, mut after_quote) = (
        Vec::new(),
        String::new(),
        s.chars().peekable(),
        false,
        false,
    );
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"')
                } else {
                    quoted = false;
                    after_quote = true
                }
            } else {
                field.push(c)
            }
            continue;
        }
        if after_quote && !matches!(c, ',' | '\r' | '\n') {
            return Err("Invalid CSV: unexpected character after closing quote.".into());
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            ',' => {
                row.push(std::mem::take(&mut field));
                after_quote = false
            }
            '\n' | '\r' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                after_quote = false
            }
            _ => field.push(c),
        }
    }
    if quoted {
        return Err("Invalid CSV: unterminated quoted field.".into());
    }
    if !field.is_empty() || !row.is_empty() || s.is_empty() || !s.ends_with(['\n', '\r']) {
        row.push(field);
        rows.push(row)
    }
    Ok(rows)
}

fn logging(name: &str, a: &[Value]) -> Result<Value, String> {
    arity(&format!("Logging.{}", name), a, 1)?;
    let msg = a[0].to_string_value();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match name {
        "info" => println!("[{}] INFO {}", stamp, msg),
        "warning" => eprintln!("[{}] WARNING {}", stamp, msg),
        "error" => eprintln!("[{}] ERROR {}", stamp, msg),
        _ => return Err(format!("Logging has no function '{}'.", name)),
    }
    Ok(Value::None)
}

const HTTP_RESPONSE_LIMIT: u64 = 16 * 1024 * 1024;
fn http_client(
    url: &reqwest::Url,
    timeout: std::time::Duration,
    max_redirects: usize,
) -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(max_redirects));
    if url.host_str().is_some_and(|h| {
        h.eq_ignore_ascii_case("localhost")
            || h.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    }) {
        builder = builder.no_proxy()
    }
    builder
        .build()
        .map_err(|e| format!("Cannot create HTTP client: {}", e))
}
fn checked_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|e| format!("Invalid URL: {}", e))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("HTTP accepts only http:// and https:// URLs.".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Credentials embedded in URLs are not supported; use request headers.".into());
    }
    Ok(url)
}
fn http(name: &str, a: &[Value]) -> Result<Value, String> {
    if name == "download" {
        arity("HTTP.download", a, 2)?;
        let raw = string("HTTP.download", &a[0])?;
        let url = checked_url(raw)?;
        let dest = std::path::Path::new(string("HTTP.download", &a[1])?);
        if dest.exists() {
            return Err(format!(
                "Download destination '{}' already exists.",
                dest.display()
            ));
        }
        let client = http_client(&url, std::time::Duration::from_secs(30), 5)?;
        let mut response = client
            .get(url.clone())
            .send()
            .map_err(|e| format!("HTTP download failed: {}", e))?;
        if !response.status().is_success() {
            return Err(format!(
                "HTTP download returned status {}.",
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|n| n > 100 * 1024 * 1024)
        {
            return Err("HTTP download exceeds the 100 MiB size limit.".into());
        }
        let parent = dest
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let filename = dest
            .file_name()
            .ok_or("Download destination must name a file.")?;
        let temp = parent.join(format!(
            ".{}.nova-download-{}",
            filename.to_string_lossy(),
            std::process::id()
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| format!("Cannot create temporary download file: {}", e))?;
        let result = (|| {
            let mut limited = (&mut response).take(100 * 1024 * 1024 + 1);
            let copied = std::io::copy(&mut limited, &mut file)
                .map_err(|e| format!("Download failed: {}", e))?;
            if copied > 100 * 1024 * 1024 {
                return Err("HTTP download exceeds the 100 MiB size limit.".into());
            }
            file.sync_all()
                .map_err(|e| format!("Cannot flush downloaded file: {}", e))?;
            fs::hard_link(&temp, dest)
                .or_else(|_| {
                    if dest.exists() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::AlreadyExists,
                            "destination exists",
                        ));
                    }
                    fs::rename(&temp, dest)
                })
                .map_err(|e| format!("Cannot finalize download: {}", e))?;
            Ok(Value::Int(copied as i64))
        })();
        let _ = fs::remove_file(&temp);
        return result;
    }
    let (method, index, body) = match name {
        "get" => {
            arity("HTTP.get", a, 1)?;
            (reqwest::Method::GET, 0, None)
        }
        "post" => {
            arity("HTTP.post", a, 2)?;
            (reqwest::Method::POST, 0, Some(string("HTTP.post", &a[1])?))
        }
        _ => return Err(format!("HTTP has no function '{}'.", name)),
    };
    let url = checked_url(string(&format!("HTTP.{}", name), &a[index])?)?;
    let client = http_client(&url, std::time::Duration::from_secs(15), 5)?;
    let mut request = client.request(method, url.clone());
    if let Some(body) = body {
        request = request.body(body.to_owned());
    }
    let mut response = request
        .send()
        .map_err(|e| format!("HTTP request failed: {:#}", e))?;
    if response
        .content_length()
        .is_some_and(|n| n > HTTP_RESPONSE_LIMIT)
    {
        return Err("HTTP response exceeds the 16 MiB size limit.".into());
    }
    let status = response.status().as_u16();
    let final_url = response.url().to_string();
    let headers = response
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                Value::String(k.to_string()),
                Value::String(v.to_str().unwrap_or("<non-text>").to_string()),
            )
        })
        .collect();
    let mut bytes = Vec::new();
    (&mut response)
        .take(HTTP_RESPONSE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Cannot read HTTP response: {}", e))?;
    if bytes.len() as u64 > HTTP_RESPONSE_LIMIT {
        return Err("HTTP response exceeds the 16 MiB size limit.".into());
    }
    let text = String::from_utf8(bytes).map_err(
        |_| "HTTP response body is not valid UTF-8; binary response access is not implemented.",
    )?;
    Ok(Value::Dictionary(vec![
        (Value::from("status"), Value::Int(status as i64)),
        (Value::from("body"), Value::String(text)),
        (Value::from("headers"), Value::Dictionary(headers)),
        (Value::from("url"), Value::String(final_url)),
    ]))
}
fn network(name: &str, a: &[Value]) -> Result<Value, String> {
    match name {
        "resolve" => {
            arity("Network.resolve", a, 1)?;
            let host = string("Network.resolve", &a[0])?;
            let addresses = std::net::ToSocketAddrs::to_socket_addrs(&(host, 0))
                .map_err(|e| format!("Cannot resolve host '{}': {}", host, e))?;
            let mut values = Vec::new();
            for addr in addresses {
                let s = addr.ip().to_string();
                if !values.iter().any(|v| v == &Value::String(s.clone())) {
                    values.push(Value::String(s));
                }
            }
            Ok(Value::List(values))
        }
        _ => Err(format!("Network has no function '{}'.", name)),
    }
}

fn datetime(name: &str, a: &[Value]) -> Result<Value, String> {
    match name {
        "now" => {
            arity("DateTime.now", a, 0)?;
            let s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("System clock error: {}", e))?
                .as_secs();
            Ok(Value::Int(
                i64::try_from(s).map_err(|_| "Timestamp exceeds Nova Int range.")?,
            ))
        }
        "today" => {
            arity("DateTime.today", a, 0)?;
            let s = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("System clock error: {}", e))?
                .as_secs() as i64;
            Ok(Value::String(format_date(s, "%Y-%m-%d")))
        }
        "parse" => {
            arity("DateTime.parse", a, 1)?;
            let text = string("DateTime.parse", &a[0])?;
            parse_iso(text).map(Value::Int)
        }
        "format" => {
            arity("DateTime.format", a, 2)?;
            let stamp = int("DateTime.format", &a[0])?;
            let fmt = string("DateTime.format", &a[1])?;
            Ok(Value::String(format_date(stamp, fmt)))
        }
        _ => Err(format!("DateTime has no function '{}'.", name)),
    }
}
fn format_date(timestamp: i64, fmt: &str) -> String {
    let days = timestamp.div_euclid(86400);
    let secs = timestamp.rem_euclid(86400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs / 3600;
    let minute = (secs % 3600) / 60;
    let second = secs % 60;
    fmt.replace("%Y", &format!("{:04}", year))
        .replace("%m", &format!("{:02}", month))
        .replace("%d", &format!("{:02}", day))
        .replace("%H", &format!("{:02}", hour))
        .replace("%M", &format!("{:02}", minute))
        .replace("%S", &format!("{:02}", second))
}
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    if m <= 2 {
        y += 1
    }
    (y, m, d)
}
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
fn parse_iso(text: &str) -> Result<i64, String> {
    let clean = text.strip_suffix('Z').unwrap_or(text);
    let (date, time) = clean.split_once('T').unwrap_or((clean, "00:00:00"));
    let mut d = date.split('-');
    let year = d.next().and_then(|x| x.parse::<i64>().ok());
    let month = d.next().and_then(|x| x.parse::<i64>().ok());
    let day = d.next().and_then(|x| x.parse::<i64>().ok());
    if d.next().is_some() {
        return Err(
            "DateTime.parse expects ISO date/time like YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ.".into(),
        );
    }
    let (Some(y), Some(m), Some(day)) = (year, month, day) else {
        return Err(
            "DateTime.parse expects ISO date/time like YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ.".into(),
        );
    };
    if !(1..=12).contains(&m) || day < 1 || day > days_in_month(y, m) {
        return Err("DateTime.parse received an invalid calendar date.".into());
    }
    let mut t = time.split(':');
    let h = t
        .next()
        .and_then(|x| x.parse::<i64>().ok())
        .ok_or("Invalid ISO time.")?;
    let min = t
        .next()
        .and_then(|x| x.parse::<i64>().ok())
        .ok_or("Invalid ISO time.")?;
    let sec = t
        .next()
        .and_then(|x| x.parse::<i64>().ok())
        .ok_or("Invalid ISO time.")?;
    if t.next().is_some()
        || !(0..24).contains(&h)
        || !(0..60).contains(&min)
        || !(0..60).contains(&sec)
    {
        return Err("DateTime.parse received an invalid UTC time.".into());
    }
    days_from_civil(y, m, day)
        .checked_mul(86400)
        .and_then(|n| n.checked_add(h * 3600 + min * 60 + sec))
        .ok_or_else(|| "DateTime timestamp is outside Nova Int range.".into())
}
fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 31,
    }
}

fn process_api(module: &str, name: &str, a: &[Value]) -> Result<Value, String> {
    let label = format!("{}.{}", module, name);
    match name {
        "list" => {
            arity(&label, a, 0)?;
            Ok(Value::List(
                list_processes()?.into_iter().map(process_value).collect(),
            ))
        }
        "find" | "info" | "exists" => {
            arity(&label, a, 1)?;
            let query = &a[0];
            let all = list_processes()?;
            let matches: Vec<_> = all
                .into_iter()
                .filter(|p| match query {
                    Value::Int(pid) => p.pid == *pid,
                    Value::String(q) => p.name.eq_ignore_ascii_case(q),
                    _ => false,
                })
                .collect();
            if name == "exists" {
                return Ok(Value::Bool(!matches.is_empty()));
            }
            if name == "find" {
                return Ok(Value::List(
                    matches.into_iter().map(process_value).collect(),
                ));
            }
            match matches.len() {
                0 => Err(format!("{} found no matching process.", label)),
                1 => Ok(process_value(matches.into_iter().next().unwrap())),
                _ => Err(format!(
                    "{} matched multiple processes; use find() and select a PID.",
                    label
                )),
            }
        }
        "is_running" => {
            arity(&label, a, 1)?;
            let pid = int(&label, &a[0])?;
            Ok(Value::Bool(list_processes()?.iter().any(|p| p.pid == pid)))
        }
        "start" => {
            if a.is_empty() || a.len() > 2 {
                return Err(
                    "Process.start expects a program path and optional List[String] arguments."
                        .into(),
                );
            }
            let program = string(&label, &a[0])?;
            let mut cmd = std::process::Command::new(program);
            if a.len() == 2 {
                let Value::List(args) = &a[1] else {
                    return Err("Process.start arguments must be a List[String].".into());
                };
                for arg in args {
                    cmd.arg(string(&label, arg)?);
                }
            }
            let child = cmd
                .spawn()
                .map_err(|e| format!("Cannot start process '{}': {}", program, e))?;
            Ok(Value::Int(child.id() as i64))
        }
        _ => Err(format!("{} has no function '{}'.", module, name)),
    }
}
struct ProcessInfo {
    pid: i64,
    name: String,
}
fn process_value(p: ProcessInfo) -> Value {
    Value::Dictionary(vec![
        (Value::from("pid"), Value::Int(p.pid)),
        (Value::from("name"), Value::String(p.name)),
        (Value::from("running"), Value::Bool(true)),
    ])
}
#[cfg(windows)]
fn list_processes() -> Result<Vec<ProcessInfo>, String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Process | Select-Object ProcessName,Id | ConvertTo-Csv -NoTypeInformation",
        ])
        .output()
        .map_err(|e| format!("Cannot list processes: {}", e))?;
    if !output.status.success() {
        return Err("PowerShell Get-Process failed while listing processes.".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let f = parse_tasklist_csv(line);
        if f.len() >= 2 {
            if let Ok(pid) = f[1].parse() {
                out.push(ProcessInfo {
                    pid,
                    name: f[0].clone(),
                })
            }
        }
    }
    Ok(out)
}
#[cfg(unix)]
fn list_processes() -> Result<Vec<ProcessInfo>, String> {
    let output = std::process::Command::new("ps")
        .args(["-e", "-o", "pid=,comm="])
        .output()
        .map_err(|e| format!("Cannot list processes: {}", e))?;
    if !output.status.success() {
        return Err("The ps command failed while listing processes.".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            Some(ProcessInfo {
                pid,
                name: parts.collect::<Vec<_>>().join(" "),
            })
        })
        .collect())
}
#[cfg(not(any(windows, unix)))]
fn list_processes() -> Result<Vec<ProcessInfo>, String> {
    Err("Process listing is unsupported on this operating system.".into())
}
#[cfg(windows)]
fn parse_tasklist_csv(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut current, mut quoted) = (String::new(), false);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                current.push('"')
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    out.push(current);
    out
}

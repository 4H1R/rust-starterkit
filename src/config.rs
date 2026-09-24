use std::{net::SocketAddr, time::Duration};

/// Intentionally no Debug implementation: the database URL contains credentials.
#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind: SocketAddr,
    pub enable_example: bool,
    pub db_max_connections: u32,
    pub request_timeout: Duration,
    pub body_limit: usize,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let database_url = get("DATABASE_URL").ok_or("DATABASE_URL is required")?;
        let url =
            url::Url::parse(&database_url).map_err(|_| "DATABASE_URL must be a PostgreSQL URL")?;
        if !matches!(url.scheme(), "postgres" | "postgresql") || url.host_str().is_none() {
            return Err("DATABASE_URL must be a PostgreSQL URL".into());
        }
        let bind = get("BIND_ADDR")
            .unwrap_or_else(|| "127.0.0.1:3000".into())
            .parse()
            .map_err(|_| "BIND_ADDR must be an IP socket address")?;
        let enable_example = get("ENABLE_EXAMPLE")
            .unwrap_or_else(|| "false".into())
            .parse()
            .map_err(|_| "ENABLE_EXAMPLE must be true or false")?;
        let number = |name, default, max| -> Result<u64, String> {
            let value = get(name)
                .unwrap_or_else(|| format!("{default}"))
                .parse::<u64>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if value == 0 || value > max {
                return Err(format!("{name} must be between 1 and {max}"));
            }
            Ok(value)
        };
        Ok(Self {
            database_url,
            bind,
            enable_example,
            db_max_connections: number("DB_MAX_CONNECTIONS", 10, 100)? as u32,
            request_timeout: Duration::from_millis(number("REQUEST_TIMEOUT_MS", 10000, 120000)?),
            body_limit: number("BODY_LIMIT_BYTES", 16384, 1048576)? as usize,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_is_validated_without_echoing_secrets() {
        let config = Config::from_lookup(|key| {
            (key == "DATABASE_URL").then(|| "postgres://u:secret@localhost/db".into())
        })
        .unwrap();
        assert!(!config.enable_example);
        assert_eq!(config.body_limit, 16384);
        for (key, value) in [
            ("DATABASE_URL", "bad-secret"),
            ("ENABLE_EXAMPLE", "yes"),
            ("BODY_LIMIT_BYTES", "0"),
            ("DB_MAX_CONNECTIONS", "101"),
            ("BIND_ADDR", "bad"),
        ] {
            let error = Config::from_lookup(|k| {
                if k == key {
                    Some(value.into())
                } else if k == "DATABASE_URL" {
                    Some("postgres://u:secret@localhost/db".into())
                } else {
                    None
                }
            })
            .err()
            .unwrap();
            assert!(!error.contains("secret"));
        }
    }
}

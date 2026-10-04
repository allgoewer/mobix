//! Settings from `$MOBIX_CONFIG` (`/boot/mobix.conf`, shell `KEY=value` syntax).

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

pub struct Config {
    pub hostname: String,
    pub mqtt: Option<Mqtt>,
    pub printer_uri: String,
    /// Fixed render resolution; `None` asks the printer
    pub print_ppi: Option<u32>,
}

pub struct Mqtt {
    pub host: String,
    pub port: u16,
    pub credentials: Option<(String, String)>,
    pub tls: bool,
    /// PEM file with the CA that signed the broker certificate
    pub ca: Option<PathBuf>,
    pub topic_prefix: String,
}

impl Config {
    pub fn load() -> Result<Config> {
        let path = std::env::var("MOBIX_CONFIG").unwrap_or_else(|_| "/boot/mobix.conf".into());
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?;
        Config::parse(&text, &system_hostname())
    }

    fn parse(text: &str, default_hostname: &str) -> Result<Config> {
        let vars = parse_vars(text);
        let get = |key: &str| vars.get(key).filter(|v| !v.is_empty()).cloned();

        let hostname = get("HOSTNAME").unwrap_or_else(|| default_hostname.to_string());

        let mqtt = match get("MQTT_HOST") {
            None => None,
            Some(host) => {
                let tls = matches!(get("MQTT_TLS").as_deref(), Some("1" | "yes" | "true"));
                let port = match get("MQTT_PORT") {
                    Some(p) => p
                        .parse()
                        .with_context(|| format!("invalid MQTT_PORT {p:?}"))?,
                    None if tls => 8883,
                    None => 1883,
                };
                let credentials = match (get("MQTT_USER"), get("MQTT_PASSWORD")) {
                    (Some(user), password) => Some((user, password.unwrap_or_default())),
                    (None, Some(_)) => bail!("MQTT_PASSWORD is set without MQTT_USER"),
                    (None, None) => None,
                };
                let topic_prefix = get("MQTT_TOPIC_PREFIX")
                    .unwrap_or_else(|| format!("mobix/{hostname}"))
                    .trim_end_matches('/')
                    .to_string();
                Some(Mqtt {
                    host,
                    port,
                    credentials,
                    tls,
                    ca: get("MQTT_CA").map(PathBuf::from),
                    topic_prefix,
                })
            }
        };

        let print_ppi = match get("PRINT_PPI") {
            Some(p) => Some(
                p.parse()
                    .with_context(|| format!("invalid PRINT_PPI {p:?}"))?,
            ),
            None => None,
        };

        Ok(Config {
            hostname,
            mqtt,
            printer_uri: get("PRINTER_URI")
                .unwrap_or_else(|| "ipp://localhost:8000/ipp/print".into()),
            print_ppi,
        })
    }
}

fn system_hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|h| h.trim().to_string())
        .unwrap_or_else(|_| "mobix".into())
}

/// Parses `KEY=value` lines with optional `export`, quotes and `#` comments.
/// No variable expansion: the file is also read by `/bin/sh`, keep it simple.
fn parse_vars(text: &str) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        vars.insert(key.to_string(), unquote(value));
    }
    vars
}

fn unquote(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => out.extend(chars.by_ref().take_while(|&c| c != '\'')),
            '"' => {
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => out.extend(chars.next()),
                        c => out.push(c),
                    }
                }
            }
            '\\' => out.extend(chars.next()),
            // Unquoted whitespace ends the value, a comment may follow
            c if c.is_whitespace() => break,
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_syntax() {
        let vars = parse_vars(
            "# comment\nA=plain\nexport B=\"two words\" # trailing\nC='it''s'\nD=a\\ b\n  E= \nnot a var\n",
        );
        assert_eq!(vars["A"], "plain");
        assert_eq!(vars["B"], "two words");
        assert_eq!(vars["C"], "its");
        assert_eq!(vars["D"], "a b");
        assert_eq!(vars["E"], "");
        assert_eq!(vars.len(), 5);
    }

    #[test]
    fn defaults() {
        let c = Config::parse("HOSTNAME=box\nMQTT_HOST=broker\n", "x").unwrap();
        let m = c.mqtt.unwrap();
        assert_eq!(
            (m.port, m.tls, m.topic_prefix.as_str()),
            (1883, false, "mobix/box")
        );
        assert_eq!(c.printer_uri, "ipp://localhost:8000/ipp/print");
        assert!(c.print_ppi.is_none());

        let c = Config::parse("MQTT_HOST=b\nMQTT_TLS=1\nMQTT_TOPIC_PREFIX=a/b/\n", "x").unwrap();
        let m = c.mqtt.unwrap();
        assert_eq!((m.port, m.topic_prefix.as_str()), (8883, "a/b"));

        assert!(Config::parse("NTP_SERVER=x\n", "x").unwrap().mqtt.is_none());
    }
}

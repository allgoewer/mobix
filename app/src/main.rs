//! mobix-app: prints MQTT messages on the receipt printer.
//!
//! Subscribes to `<prefix>/print` and renders each message (JSON with text
//! and/or a base64 image) with Typst on 80 mm paper, then hands the PNG to
//! LPrint over IPP. `<prefix>/status` is a retained `online`/`offline` flag,
//! `offline` being the last will.

mod config;
mod print;
mod render;
mod syslog;

use std::collections::HashSet;
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use log::{error, info, warn};
use rumqttc::{
    Client, Event, LastWill, MqttOptions, Outgoing, Packet, Publish, QoS, TlsConfiguration,
    Transport,
};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

use config::Config;
use print::{PrintError, Printer};
use render::Job;

/// Largest accepted message, roughly a 6 MB image after base64
const MAX_PAYLOAD: usize = 8 * 1024 * 1024;

/// Print jobs are not idempotent, so they are acknowledged only after they
/// were handed to LPrint (or dropped as invalid). With QoS 1 and a persistent
/// session, the broker keeps unacknowledged jobs across reconnects and
/// restarts; a job is printed twice only if the app dies between submitting
/// it and acknowledging it. QoS 2 would not narrow that window and rumqttc
/// does not keep QoS 2 state across reconnects.
const PRINT_QOS: QoS = QoS::AtLeastOnce;

fn main() -> ExitCode {
    syslog::init();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let config = Config::load()?;
    let Some(mqtt) = &config.mqtt else {
        info!("MQTT_HOST is not set in mobix.conf, nothing to do");
        // Exiting would only make init restart the app over and over
        loop {
            thread::park();
        }
    };
    let printer = Printer::new(&config.printer_uri)?;

    let print_topic = format!("{}/print", mqtt.topic_prefix);
    let status_topic = format!("{}/status", mqtt.topic_prefix);

    // A fixed client id keeps the session (and queued jobs) across restarts
    let mut options = MqttOptions::new(format!("mobix-{}", config.hostname), &mqtt.host, mqtt.port);
    options
        .set_keep_alive(Duration::from_secs(30))
        .set_clean_session(false)
        .set_manual_acks(true)
        .set_max_packet_size(MAX_PAYLOAD, 64 * 1024)
        .set_last_will(LastWill::new(
            &status_topic,
            "offline",
            QoS::AtLeastOnce,
            true,
        ));
    if let Some((user, password)) = &mqtt.credentials {
        options.set_credentials(user, password);
    }
    if mqtt.tls {
        let tls = match &mqtt.ca {
            Some(path) => TlsConfiguration::SimpleNative {
                ca: std::fs::read(path).with_context(|| format!("reading {}", path.display()))?,
                client_auth: None,
            },
            None => TlsConfiguration::Native,
        };
        options.set_transport(Transport::tls_with_config(tls));
    }

    let (client, mut connection) = Client::new(options, 16);

    // Say goodbye on SIGTERM so the status flips to offline right away
    let mut signals = Signals::new([SIGTERM, SIGINT])?;
    {
        let client = client.clone();
        let status_topic = status_topic.clone();
        thread::spawn(move || {
            if signals.forever().next().is_some() {
                info!("shutting down");
                let _ = client.try_publish(&status_topic, QoS::AtLeastOnce, true, "offline");
                let _ = client.try_disconnect();
                // Not connected: the broker publishes the last will anyway
                thread::sleep(Duration::from_secs(3));
                std::process::exit(0);
            }
        });
    }

    // Message ids that are queued or printing, to skip redeliveries of them
    // after a reconnect
    let pending = Arc::new(Mutex::new(HashSet::new()));
    let (jobs, queue) = mpsc::channel();
    {
        let client = client.clone();
        let pending = pending.clone();
        let ppi = config.print_ppi;
        thread::spawn(move || worker(queue, client, printer, ppi, pending));
    }

    info!(
        "connecting to {}:{}{}",
        mqtt.host,
        mqtt.port,
        if mqtt.tls { " (TLS)" } else { "" }
    );
    let mut backoff = Duration::from_secs(1);
    for event in connection.iter() {
        match event {
            Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                info!("connected, session present: {}", ack.session_present);
                backoff = Duration::from_secs(1);
                // try_*: this thread drives the event loop, blocking here could deadlock
                client.try_subscribe(&print_topic, PRINT_QOS)?;
                if let Err(e) = client.try_publish(&status_topic, QoS::AtLeastOnce, true, "online")
                {
                    warn!("publishing status: {e}");
                }
            }
            Ok(Event::Incoming(Packet::Publish(publish))) => {
                if publish.topic != print_topic {
                    // Left over in a session from an older configuration
                    info!("ignoring message on {}", publish.topic);
                    if let Err(e) = client.try_ack(&publish) {
                        warn!("ack of message {}: {e}", publish.pkid);
                    }
                    continue;
                }
                if publish.qos != QoS::AtMostOnce && !pending.lock().unwrap().insert(publish.pkid) {
                    info!(
                        "message {} is already queued, ignoring redelivery",
                        publish.pkid
                    );
                    continue;
                }
                jobs.send(publish)?;
            }
            Ok(Event::Outgoing(Outgoing::Disconnect)) => break,
            Ok(_) => {}
            Err(e) => {
                warn!("MQTT connection: {e}");
                thread::sleep(backoff);
                backoff = (backoff * 2).min(Duration::from_secs(30));
            }
        }
    }
    Ok(())
}

/// Prints jobs one at a time in arrival order
fn worker(
    queue: Receiver<Publish>,
    client: Client,
    printer: Printer,
    ppi: Option<u32>,
    pending: Arc<Mutex<HashSet<u16>>>,
) {
    for publish in queue {
        if let Err(e) = process(&publish, &printer, ppi) {
            // Dropped: the same message would fail again
            error!("message on {} dropped: {e:#}", publish.topic);
        }
        if let Err(e) = client.ack(&publish) {
            warn!("ack of message {}: {e}", publish.pkid);
        }
        pending.lock().unwrap().remove(&publish.pkid);
    }
}

fn process(publish: &Publish, printer: &Printer, ppi: Option<u32>) -> Result<()> {
    let job = Job::parse(&publish.payload)?;
    let ppi = match ppi {
        Some(ppi) => ppi,
        None => retry(|| printer.resolution())?,
    };
    let page = render::render(&job, ppi)?;
    let height_hmm = page.height_hmm();
    let title = job
        .text
        .as_deref()
        .and_then(|t| t.lines().next())
        .unwrap_or("image");
    let title: String = title.chars().take(60).collect();
    let job_id = retry(|| printer.print(&title, page.png.clone(), height_hmm))?;
    info!(
        "printing job {job_id}: 80x{:.0} mm",
        f64::from(height_hmm) / 100.0
    );
    Ok(())
}

/// Retries until LPrint is reachable again; holds back later jobs meanwhile
fn retry<T>(mut f: impl FnMut() -> Result<T, PrintError>) -> Result<T> {
    let mut delay = Duration::from_secs(1);
    loop {
        match f() {
            Ok(v) => return Ok(v),
            Err(PrintError::Rejected(e)) => return Err(e),
            Err(PrintError::Retry(e)) => {
                warn!("{e:#}, retrying in {} s", delay.as_secs());
                thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_secs(30));
            }
        }
    }
}

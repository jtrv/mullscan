use clap::{App, Arg};
use reqwest::Error;
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashSet;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task;

#[derive(Debug, Deserialize, Clone)]
struct ServerData {
    hostname: String,
    country_code: String,
    country_name: String,
    city_code: Option<String>,
    city_name: String,
    active: bool,
    owned: bool,
    provider: String,
    ipv4_addr_in: String,
    ipv6_addr_in: Option<String>,
    network_port_speed: u32,
    stboot: bool,
    #[serde(rename = "type")]
    server_type: Option<String>,
    status_messages: Option<Vec<StatusMessage>>,
    pubkey: Option<String>,
    multihop_port: Option<u16>,
    socks_name: Option<String>,
    socks_port: Option<u16>,
}

#[derive(Debug, Deserialize, Clone)]
struct StatusMessage {
    message: String,
    timestamp: String,
}

#[derive(Debug, Clone)]
struct ResultData {
    hostname: String,
    city: String,
    country: String,
    server_type: Option<String>,
    ip: String,
    avg: f64,
    network_port_speed: u32,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let matches = App::new("mullscan")
        .arg(
            Arg::new("country")
                .short('c')
                .long("country")
                .value_name("code")
                .help("The country you want to query (e.g., us, gb, de)")
                .takes_value(true),
        )
        .arg(
            Arg::new("list_countries")
                .short('l')
                .long("list-countries")
                .help("Lists the available countries"),
        )
        .arg(
            Arg::new("server_type")
                .short('t')
                .long("type")
                .value_name("type")
                .help("The type of server to query (openvpn, bridge, wireguard, all)")
                .default_value("all")
                .takes_value(true),
        )
        .arg(
            Arg::new("pings")
                .short('p')
                .long("pings")
                .value_name("n")
                .help("The number of pings to the server (default 3)")
                .default_value("3")
                .takes_value(true),
        )
        .arg(
            Arg::new("interval")
                .short('i')
                .long("interval")
                .value_name("seconds")
                .help("The interval between pings in seconds (default/min 0.2)")
                .default_value("0.2")
                .takes_value(true),
        )
        .arg(
            Arg::new("count")
                .short('n')
                .long("count")
                .value_name("n")
                .help("The number of top servers to show, (0=all)")
                .default_value("5")
                .takes_value(true),
        )
        .arg(
            Arg::new("port_speed")
                .short('s')
                .long("port-speed")
                .value_name("Gbps")
                .help("Only show servers with at least n Gigabit port speed")
                .default_value("1")
                .takes_value(true),
        )
        .arg(
            Arg::new("run_mode")
                .short('r')
                .long("run-mode")
                .value_name("mode")
                .help("Only show servers running from (all, ram, disk)")
                .default_value("all")
                .takes_value(true),
        )
        .get_matches();

    let country = matches.value_of("country").map(|c| c.to_owned());
    let server_type = matches.value_of("server_type").unwrap().to_owned();
    let interval = matches
        .value_of("interval")
        .unwrap()
        .parse::<f64>()
        .unwrap_or(0.2);
    let pings = matches
        .value_of("pings")
        .unwrap()
        .parse::<usize>()
        .unwrap_or(3);
    let top_n = matches
        .value_of("count")
        .unwrap()
        .parse::<usize>()
        .unwrap_or(5);
    let port_speed = matches
        .value_of("port_speed")
        .unwrap()
        .parse::<u32>()
        .unwrap_or(0);
    let run_mode = matches.value_of("run_mode").unwrap().to_owned();

    let server_data = fetch_server_data(&server_type).await?;

    if matches.is_present("list_countries") {
        list_countries(&server_data);
    } else {
        let (tx, mut rx) = mpsc::channel::<ResultData>(10);

        for server in server_data {
            let tx = tx.clone();
            let server = server.clone();
            let country = country.clone();
            let run_mode = run_mode.clone();
            task::spawn(async move {
                if let Some(result) =
                    find_best_server(&server, &country, port_speed, &run_mode, pings, interval)
                        .await
                {
                    let _ = tx.send(result).await;
                }
            });
        }

        let mut results = Vec::new();
        drop(tx);

        while let Some(result) = rx.recv().await {
            results.push(result);
        }

        results.sort_by(|a, b| a.avg.partial_cmp(&b.avg).unwrap_or(Ordering::Equal));
        results.truncate(top_n);
        display_top_servers(&results, top_n);
    }

    Ok(())
}

async fn fetch_server_data(server_type: &str) -> Result<Vec<ServerData>, Error> {
    let url = format!("https://api.mullvad.net/www/relays/{}/", server_type);
    let response = reqwest::get(url).await?;
    let server_data: Vec<ServerData> = response.json().await?;
    Ok(server_data)
}

fn list_countries(server_data: &[ServerData]) {
    let mut countries = HashSet::new();
    for server in server_data {
        countries.insert((server.country_code.clone(), server.country_name.clone()));
    }

    let mut countries_vec: Vec<(String, String)> = countries.into_iter().collect();

    countries_vec.sort_by(|a, b| a.1.cmp(&b.1));

    for (code, name) in countries_vec {
        println!("{} - {}", code, name);
    }
}

async fn find_best_server(
    server: &ServerData,
    country: &Option<String>,
    port_speed: u32,
    run_mode: &str,
    pings: usize,
    interval: f64,
) -> Option<ResultData> {
    if (country.is_none() || country.as_ref().unwrap() == &server.country_code)
        && (server.network_port_speed >= port_speed)
        && check_run_mode(server.stboot, run_mode)
    {
        let avg = ping(&server.ipv4_addr_in, pings, interval).await;
        if let Some(avg) = avg {
            return Some(ResultData {
                hostname: server.hostname.clone(),
                city: server.city_name.clone(),
                country: server.country_name.clone(),
                server_type: server.server_type.clone(),
                ip: server.ipv4_addr_in.clone(),
                avg,
                network_port_speed: server.network_port_speed,
            });
        }
    }
    None
}

fn check_run_mode(server_stboot: bool, run_mode: &str) -> bool {
    match run_mode {
        "ram" => server_stboot,
        "disk" => !server_stboot,
        _ => true,
    }
}

async fn ping(ip: &str, pings: usize, interval: f64) -> Option<f64> {
    let output = Command::new("ping")
        .arg("-c")
        .arg(pings.to_string())
        .arg("-i")
        .arg(interval.to_string())
        .arg(ip)
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let re = regex::Regex::new(r"rtt min/avg/max/mdev = [0-9.]+/([0-9.]+)").unwrap();
        if let Some(captures) = re.captures(&stdout) {
            return captures
                .get(1)
                .map(|m| m.as_str().parse::<f64>().unwrap_or(0.0));
        }
    }
    None
}

fn display_top_servers(results: &[ResultData], top_n: usize) {
    if !results.is_empty() {
        println!("\nTop {} results:", top_n);
        for result in results {
            let server_type: Option<&str> = result.server_type.as_deref();

            println!(
                " - {} ({:.1}ms) {} Gbps {} {}, {}",
                result.hostname,
                result.avg,
                result.network_port_speed,
                server_type.unwrap_or("unknown"),
                result.city,
                result.country
            );
        }
    } else {
        eprintln!("No servers found");
    }
}

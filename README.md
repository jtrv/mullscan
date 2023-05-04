# Mullscan

Mullscan is a CLI tool that finds the best servers from Mullvad VPN based on their latency, country, server type, port speed, and run mode.

## Usage

First, install the project using the following command:

```
cargo install --git https://github.com/jtrv/mullscan
```

Then, run the compiled binary with the desired options:

```
mullscan [OPTIONS]
```

### Options:

- `-c, --country <code>`: Filter servers by country (e.g., us, gb, de)
- `-l, --list-countries`: List available countries
- `-t, --type <type>`: Filter servers by type (openvpn, bridge, wireguard, all). Default: all
- `-p, --pings <n>`: Number of pings to each server. Default: 3
- `-i, --interval <seconds>`: Interval between pings in seconds. Default: 0.2
- `-n, --count <n>`: Number of top servers to display (0 = all). Default: 5
- `-s, --port-speed <Gbps>`: Filter servers by minimum port speed. Default: 1
- `-r, --run-mode <mode>`: Filter servers by run mode (all, ram, disk). Default: all

## Examples

1. List available countries:

```
mullscan -l
```

2. Find the best 5 servers in the United States:

```
mullscan -c us -n 5
```

3. Find the best WireGuard servers with at least 5 Gbps port speed:

```
mullscan -t wireguard -s 5
```

4. Find the best servers running from RAM in Germany:

```
mullscan -c de -r ram
```

## Todo

- [ ] use the ping crate instead of the ping command
  - once [this PR](https://github.com/aisk/ping/pull/7) is merged, we can probably gain some performance here

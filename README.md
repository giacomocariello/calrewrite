# calrewrite

A lightweight HTTP proxy that fetches an iCal (ICS) feed from an upstream URL and returns it with all event times shifted by a configurable number of seconds.

## Why?

Some calendar feeds have incorrect timestamps — off by a fixed offset due to timezone misconfiguration, DST bugs, or provider quirks. Rather than manually fixing events or waiting for a fix upstream, `calrewrite` sits between the source and your calendar client, applying a constant time shift to every datetime property in the feed.

Subscribe your calendar app to the `calrewrite` URL instead of the original, and the events show up at the right time.

## Usage

Start the server:

```sh
nix run            # via Nix
# or
cargo run          # via Cargo
```

### Configuration

The bind address and port are configurable via CLI flags or environment variables:

```
$ calrewrite --help
Usage: calrewrite [OPTIONS]

Options:
      --host <HOST>  Address to bind to [env: CALREWRITE_HOST=] [default: 0.0.0.0]
  -p, --port <PORT>  Port to listen on [env: CALREWRITE_PORT=] [default: 3000]
  -h, --help         Print help
  -V, --version      Print version
```

Examples:

```sh
calrewrite --host 127.0.0.1 --port 8080
CALREWRITE_PORT=8080 calrewrite
```

### Query parameters

Make a GET request with two query parameters:

| Parameter | Description |
|-----------|-------------|
| `url`     | The upstream iCal feed URL to fetch |
| `shift`   | Time shift in seconds (positive = later, negative = earlier) |

### Examples

Shift all events forward by 1 hour:

```
http://localhost:3000/?url=https://example.com/calendar.ics&shift=3600
```

Shift all events back by 30 minutes:

```
http://localhost:3000/?url=https://example.com/calendar.ics&shift=-1800
```

Subscribe to this URL from any calendar client (Google Calendar, Apple Calendar, Thunderbird, etc.) just as you would a regular ICS feed.

## What gets shifted

All standard iCal datetime properties are shifted:

`DTSTART`, `DTEND`, `DTSTAMP`, `CREATED`, `LAST-MODIFIED`, `RECURRENCE-ID`, `EXDATE`, `RDATE`, `DUE`, `COMPLETED`, `TRIGGER`

Both UTC timestamps (`20250101T120000Z`) and TZID-qualified datetimes (`DTSTART;TZID=Europe/Berlin:20250101T120000`) are handled. Date-only values (all-day events) are left untouched.

## NixOS module

The flake exports a NixOS module at `nixosModules.calrewrite`. Add it to your system configuration:

```nix
# flake.nix
{
  inputs.calrewrite.url = "github:giacomocariello/calrewrite";

  outputs = { nixpkgs, calrewrite, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        calrewrite.nixosModules.calrewrite
        {
          services.calrewrite = {
            enable = true;
            host = "127.0.0.1";
            port = 3000;
            openFirewall = false;
          };
        }
      ];
    };
  };
}
```

### Module options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enable` | bool | `false` | Enable the calrewrite service |
| `package` | package | flake default | The calrewrite package to use |
| `host` | string | `"127.0.0.1"` | Address to bind to |
| `port` | port | `3000` | Port to listen on |
| `openFirewall` | bool | `false` | Whether to open the firewall for the port |

The service runs as a hardened systemd unit with `DynamicUser`, restricted syscalls, and read-only filesystem access.

## Development

This project uses a Nix flake with [crane](https://crane.dev) for reproducible builds.

```sh
direnv allow           # activate the dev shell automatically
nix develop            # or enter manually
cargo watch -x run     # auto-restart on changes
cargo test             # run unit tests
nix build              # reproducible release build → ./result/bin/calrewrite
nix flake check        # run clippy and formatting checks
```

## License

MIT

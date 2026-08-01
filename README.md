# refinedtorrent

A BitTorrent client written from scratch, implementing the peer wire protocol and HTTP tracker communication directly from the [BitTorrent spec](https://www.bittorrent.org/beps/bep_0000.html) — no external torrent/bencode libraries doing the heavy lifting.

Built as a learning project to understand how BitTorrent actually works under the hood

## What it does

- Parses `.torrent` files
- Announces to HTTP(S) trackers and parses the compact peer list response
- Performs the BitTorrent handshake and peer wire protocol messaging
- Downloads from multiple peers concurrently, with:
  - Per-piece SHA1 hash verification against the `.torrent` file's piece hashes
  - Automatic requeueing of a piece if a peer disconnects or sends bad data mid-download, so no peer failure stalls the download
- Live terminal progress bar

## What it does not do (by design)

This was a learning project, not a full BitTorrent client:

- No DHT or Peer Exchange — peer discovery relies entirely on the torrent's HTTP tracker
- No magnet link support — only `.torrent` files
- No UDP tracker support — HTTP(S) trackers only
- Single-file torrents only — multi-file torrent support is not implemented
- Download only — no seeding/uploading

Because of this, the client works best against torrents with a healthy HTTP tracker (e.g. official Linux distro ISOs), and won't find peers for trackerless/DHT-only torrents.

## Showcase

[Video](https://github.com/user-attachments/assets/9e560b12-a7b1-4406-a1e9-5df60de7ab02) of downloading a sample.txt file and debian ISO and matching the ISO's SHA256 with the published checksum

https://github.com/user-attachments/assets/9e560b12-a7b1-4406-a1e9-5df60de7ab02

## Notes

- Debugging networking conditions that don't show up in a toy example: rate limiting/throttling, and connectivity issues caused by CGNAT (My ISP 😡)
- Verifying correctness — the final downloaded file's SHA256 matched the official published checksum exactly

## How to run

```bash
git clone https://github.com/RefinedDev/refinedtorrent.git
cd refinedtorrent
cargo run --release
```

A file dialog will pop up allowing you to select the `.torrent` file

## License

Dual-licensed under MIT or Apache-2.0

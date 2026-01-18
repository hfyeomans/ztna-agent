# Research Bookmarks

A collection of resources, documentation, and repositories accessed during the research phase for the ZTNA Agent.

## macOS Network Extensions
- **[NEPacketTunnelProvider | Apple Developer Documentation](https://developer.apple.com/documentation/networkextension/nepackettunnelprovider)**  
  Primary class for implementing the tunnel.
- **[NEPacketTunnelFlow | Apple Developer Documentation](https://developer.apple.com/documentation/networkextension/nepackettunnelflow)**  
  Class used to read/write packets from the virtual interface.
- **[NEFilterPacketProvider | Apple Developer Documentation](https://developer.apple.com/documentation/networkextension/nefilterpacketprovider)**  
  Evaluated as an alternative (rejected in favor of PacketTunnel).

## QUIC Libraries & Ecosystem
- **[Cloudflare/quiche (GitHub)](https://github.com/cloudflare/quiche)**  
  Selected library for Rust. Supports connection migration.
- **[quiche (crates.io)](https://crates.io/crates/quiche)**  
  Rust package registry entry.
- **[quinn-rs/quinn (GitHub)](https://github.com/quinn-rs/quinn)**  
  Alternative pure-Rust implementation (evaluated).
- **[saorsa-labs/ant-quic (GitHub)](https://github.com/saorsa-labs/ant-quic)**  
  Experimental fork with more P2P features.

## Zero Trust, NAT Traversal & Multipath
- **[Moving from STUN to QUIC Address Discovery (Iroh Blog)](https://www.iroh.computer/blog/qad)**  
  Insight into replacing STUN with QUIC specific mechanisms.
- **[Iroh on QUIC Multipath](https://www.iroh.computer/blog/iroh-on-QUIC-multipath)**  
  Details on using multiple paths/addresses.
- **[Multipath QUIC - The Cloudflare Blog](https://blog.cloudflare.com/multipath-quic/)**  
  Background on Cloudflare's implementation of Connection Migration/Multipath.

## Search Queries & Investigations
<details>
<summary>Click to expand full list of search queries</summary>

- [Google: rust quiche vs quinn vpn tunnel use case](https://www.google.com/search?q=rust+quiche+vs+quinn+vpn+tunnel+use+case)
- [Google: Cloudflare quiche "Address Discovery" QUIC](https://www.google.com/search?q=Cloudflare+quiche+%22Address+Discovery%22+QUIC)
- [Google: iroh computer quic hole punching ADD_ADDRESS](https://www.google.com/search?q=iroh+computer+quic+hole+punching+ADD_ADDRESS)
- [Google: quic-go ADD_ADDRESS frame implementation status](https://www.google.com/search?q=quic-go+ADD_ADDRESS+frame+implementation+status)
- [GitHub Search: repo:cloudflare/quiche ADD_ADDRESS](https://github.com/cloudflare/quiche/issues?q=ADD_ADDRESS)
- [GitHub Search: repo:cloudflare/quiche multipath](https://github.com/search?q=repo%3Acloudflare%2Fquiche+multipath&type=issues)
</details>

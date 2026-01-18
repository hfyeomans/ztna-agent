# manual_xcode_setup_guide.md

## Prerequisites
- **Rust Core**: `packet_processor` (Created)
- **Source Code**: Swift files generated in `ztna-agent/ios-macos/`

## Step 1: Create Xcode Project
1.  Open **Xcode** -> **Create New Project**.
2.  Choose **macOS** -> **App**.
3.  Product Name: `ZtnaAgent`
4.  Organization Identifier: `com.hankyeomans` (Must match the code: `com.hankyeomans.ztna-agent`)
5.  Interface: **SwiftUI**
6.  Language: **Swift**
7.  Save it inside `ztna-agent/ios-macos/` (next to the `App` and `Extension` folders I created).

## Step 2: Add Network Extension Target
1.  In Xcode, go to **File** -> **New** -> **Target**.
2.  Choose **macOS** -> **Network Extension**.
3.  Product Name: `Extension`
4.  **IMPORTANT**: Ensure "Provider Type" is **Packet Tunnel**. (If not asked, we configure it in Info.plist later, but modern Xcode asks).
5.  Finish. When asked to "Activate" scheme, click **Activate**.

## Step 3: Import Source Files
1.  **Host App**:
    - Delete existing `ContentView.swift` and `ZtnaAgentApp.swift` (or whatever the main file is named).
    - Right click the `ZtnaAgent` folder in sidebar -> **Add Files**.
    - Select `ztna-agent/ios-macos/App/ContentView.swift`. (Ensure "Copy items if needed" is CHECKED or just reference it).
2.  **Extension**:
    - Delete the auto-generated `PacketTunnelProvider.swift`.
    - Right click `Extension` folder in sidebar -> **Add Files**.
    - Select `ztna-agent/ios-macos/Extension/PacketTunnelProvider.swift`.

## Step 4: Configure Capabilities & Entitlements
1.  **Main App Target**:
    - Signing & Capabilities -> **+ Capability** -> **Network Extensions**.
    - Check **Packet Tunnel**.
2.  **Extension Target**:
    - Signing & Capabilities -> **+ Capability** -> **Network Extensions**.
    - Check **Packet Tunnel**.

## Step 5: Link Rust Library (The Tricky Part)
1.  **Build Rust Lib**:
    - Open Terminal: `cd ztna-agent/core/packet_processor`
    - Run: `cargo build --release`
    - Confirm output at: `target/release/libpacket_processor.a`
2.  **Add to Xcode**:
    - Drag `libpacket_processor.a` into the **Frameworks & Libraries** section of the **Extension** target.
3.  **Bridging Header**:
    - **Add file to Xcode**: Right click the root project folder in Xcode -> **Add Files**. Navigate to `ztna-agent/ios-macos/Shared/PacketProcessor-Bridging-Header.h`.
    - **IMPORTANT**: Uncheck "Copy items if needed". Check "Create groups". Click Add.
    - **Build Settings**:
        - In **Extension Target** -> **Build Settings**:
        - Search for **Objective-C Bridging Header**.
        - Set path to: `$(PROJECT_DIR)/../Shared/PacketProcessor-Bridging-Header.h` (This assumes your .xcodeproj is in `ios-macos/`).
        - Search for **Library Search Paths**.
        - **RECOMMENDED**: Use the absolute path to avoid confusion. Run `pwd` in the rust release folder to get it. It will look like: `/Users/yourname/.../target/release`

## Step 6: Verify Bundle IDs
- **App**: `com.hankyeomans.ztna-agent`
- **Extension**: `com.hankyeomans.ztna-agent.Extension`
- **Code**: Check `ContentView.swift` line `protocolConfiguration.providerBundleIdentifier`. It MUST match the Extension's actual Bundle ID.

## Step 7: Run
1.  Select **ZtnaAgent** scheme.
2.  Run.
3.  Click "Install & Start".
4.  Approve the System Extension in macOS Settings (Privacy & Security).
5.  Check Console.app for "Rust" logs!

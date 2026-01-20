//! WiFi connection utilities for ESP32.
//!
//! Provides a simple interface for connecting to WiFi networks on ESP32.

use anyhow::{bail, Result};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripheral,
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use log::info;

/// Connect to a WiFi network.
///
/// This function handles the full WiFi connection process:
/// 1. Scans for available networks
/// 2. Finds the target network and its channel
/// 3. Connects with the provided credentials
/// 4. Waits for DHCP lease
///
/// # Arguments
///
/// * `ssid` - Network name (cannot be empty)
/// * `password` - Network password (empty for open networks)
/// * `modem` - ESP32 modem peripheral
/// * `sysloop` - ESP system event loop
///
/// # Returns
///
/// Returns a boxed `EspWifi` instance that must be kept alive for the connection
/// to remain active.
///
/// # Example
///
/// ```ignore
/// let wifi = connect_wifi("MyNetwork", "password123", peripherals.modem, sysloop)?;
/// // WiFi is now connected
/// // Keep `wifi` in scope to maintain connection
/// ```
pub fn connect_wifi(
    ssid: &str,
    password: &str,
    modem: impl peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
) -> Result<(Box<EspWifi<'static>>, String)> {
    if ssid.is_empty() {
        bail!("WiFi SSID cannot be empty");
    }

    let auth_method = if password.is_empty() {
        info!("WiFi password is empty, using open network");
        AuthMethod::None
    } else {
        AuthMethod::WPA2Personal
    };

    let mut esp_wifi = EspWifi::new(modem, sysloop.clone(), None)?;
    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    // Initial configuration for scanning
    wifi.set_configuration(&Configuration::Client(ClientConfiguration::default()))?;
    wifi.start()?;

    info!("Scanning for WiFi networks...");
    let ap_infos = wifi.scan()?;

    let channel = ap_infos
        .into_iter()
        .find(|ap| ap.ssid == ssid)
        .map(|ap| {
            info!("Found '{}' on channel {}", ssid, ap.channel);
            ap.channel
        });

    if channel.is_none() {
        info!("Network '{}' not found in scan, will try anyway", ssid);
    }

    // Configure connection
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().expect("SSID too long (max 32 chars)"),
        password: password
            .try_into()
            .expect("Password too long (max 64 chars)"),
        channel,
        auth_method,
        ..Default::default()
    }))?;

    info!("Connecting to '{}'...", ssid);
    wifi.connect()?;

    info!("Waiting for DHCP lease...");
    wifi.wait_netif_up()?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;
    info!("WiFi connected!");
    info!("  IP address: {}", ip_info.ip);
    info!("  Gateway:    {}", ip_info.subnet.gateway);
    info!("  Netmask:    {}", ip_info.subnet.mask);

    Ok((Box::new(esp_wifi), ip_info.ip.to_string()))
}

/// WiFi connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiStatus {
    /// Not connected to any network.
    Disconnected,
    /// Connecting to network.
    Connecting,
    /// Connected but waiting for IP.
    WaitingForIp,
    /// Fully connected with IP address.
    Connected,
}

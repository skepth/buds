//! Setting up wifi on esp32 with std implementation.
//! We also try to show the status of the connection using an rgb.

use std::time::Duration;

use esp_idf_svc::{
    hal::peripherals,
    sys::{
        wifi_mode_t_WIFI_MODE_AP, wifi_mode_t_WIFI_MODE_APSTA, wifi_mode_t_WIFI_MODE_MAX,
        wifi_mode_t_WIFI_MODE_NAN, wifi_mode_t_WIFI_MODE_NULL, wifi_mode_t_WIFI_MODE_STA,
    },
};

fn parse_wifi_mode(current_mode: u32) -> String {
    match current_mode {
        wifi_mode_t_WIFI_MODE_APSTA => "APSTA".into(),
        wifi_mode_t_WIFI_MODE_AP => "AP".into(),
        wifi_mode_t_WIFI_MODE_STA => "STA".into(),
        wifi_mode_t_WIFI_MODE_MAX => "MAX".into(),
        wifi_mode_t_WIFI_MODE_NAN => "NAN".into(),
        wifi_mode_t_WIFI_MODE_NULL => "NULL".into(),
        _ => "unexpected".into(),
    }
}

fn main() {
    // An issue in the lib requires us to call this function.
    // See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities.
    esp_idf_svc::log::EspLogger::initialize_default();

    // Get the WiFi SSID & Password from Environment Variables.
    let wifi_ssid = env!("WIFI_SSID", "Export WIFI_SSID Enviroment Variable");
    let wifi_pwd = env!("WIFI_PWD", "Export WIFI_PWD Enviroment Variable");

    // Take peripherals, System event loop & non-volatile storafe.
    let periperals = peripherals::Peripherals::take().unwrap();
    let system_event_loop = esp_idf_svc::eventloop::EspSystemEventLoop::take().unwrap();
    let nvs_storage = esp_idf_svc::nvs::EspDefaultNvsPartition::take().unwrap();

    // Now we initlialize wifi.
    let mut wifi =
        esp_idf_svc::wifi::EspWifi::new(periperals.modem, system_event_loop, Some(nvs_storage))
            .unwrap();
    log::info!("Initialized WiFi...");

    // Attempting to set wifi to blocking.
    // It looks like we get the blocking wifi by default?
    // wifi = esp_idf_svc::wifi::BlockingWifi::wrap(wifi, system_event_loop).unwrap();
    log::info!("Set up blocking wifi...");

    // Esp Wifi Configuration.
    wifi.set_configuration(&esp_idf_svc::wifi::Configuration::Client(
        esp_idf_svc::wifi::ClientConfiguration {
            ssid: wifi_ssid.try_into().unwrap(),
            password: wifi_pwd.try_into().unwrap(),
            ..Default::default()
        },
    ))
    .unwrap();
    log::info!("Set up client configuration...");

    // For curiosity, lets check the currentl Wifi mode of operation.
    // esp_wifi_get_mode takes a out param.
    // Notes: turns out STA is the default here?
    let mut current_mode: u32 = 0;
    // SAFETY: wifi_mode_t_WIFI_MODE_STA is always u32.
    let status = unsafe { esp_idf_svc::hal::sys::esp_wifi_get_mode(&mut current_mode) };
    esp_idf_svc::hal::sys::EspError::convert(status).unwrap();

    log::warn!("Current Wifi Mode: {}", parse_wifi_mode(current_mode));

    // Change WiFi Mode.
    // Available modes are:
    // AP : Access Point
    // STA : Client Mode
    // APSTA: AP + STA
    // NAN: Wi-Fi AwareTM (NAN)
    // MAX: ??
    // NULL: ??
    // In our case, we only need STA mode
    // NOTE: This is a C ABI call and needs to be wrapped in unsafe.
    // SAFETY: wifi_mode_t_WIFI_MODE_STA is always u32.
    unsafe {
        let status = esp_idf_svc::hal::sys::esp_wifi_set_mode(
            esp_idf_svc::hal::sys::wifi_mode_t_WIFI_MODE_STA,
        );
    }
    esp_idf_svc::hal::sys::EspError::convert(status).unwrap();

    // Check the new WiFi mode.
    // SAFETY: wifi_mode_t_WIFI_MODE_STA is always u32.
    unsafe {
        let status = esp_idf_svc::hal::sys::esp_wifi_get_mode(&mut current_mode);
    }
    esp_idf_svc::hal::sys::EspError::convert(status).unwrap();
    log::warn!("New Wifi Mode: {}", parse_wifi_mode(current_mode));

    wifi.start().unwrap();
    log::info!("Started the wifi...");

    // If scanning is needed for WiFi, use this functions.
    /*
    for ap in wifi.scan().unwrap() {
        if ap.ssid == "<SSID>" {
            println!("\n{:?}\n", ap);
            break;
        }
    } */

    match wifi.connect() {
        Ok(_) => log::info!("Attempting to connect to wifi..."),
        Err(e) => log::error!("Wifi connection failed: {:?}", e),
    }

    // We need to wait for the connection status to be successful.
    // Otherwise we won't be able to complete the connection.
    while !wifi.is_connected().unwrap() {
        println!("Waiting for connection: ...");
        std::thread::sleep(Duration::new(10, 0));
    }
    log::info!("Wifi Connection established");

    // sta_netif returns the client mode ip addresses.
    let net_info = wifi.sta_netif();

    log::warn!(
        "WiFi AP Status Is On ?: {}",
        wifi.ap_netif().is_up().unwrap()
    );

    loop {
        log::info!(
            "\nMAC: {:?}, IP Info: {:?}\n",
            net_info.get_mac().unwrap(),
            net_info.get_ip_info().unwrap()
        );
        std::thread::sleep(Duration::new(10, 0));
    }
}

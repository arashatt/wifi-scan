use anyhow::Error;
use embedded_svc::wifi::ClientConfiguration;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use esp_idf_svc::http::Method;
use esp_idf_svc::io::Write;
use esp_idf_svc::nvs::{EspDefaultNvs, EspDefaultNvsPartition};
use esp_idf_svc::ping::EspPing;
use esp_idf_svc::timer::EspTaskTimerService;
use esp_idf_svc::wifi::AsyncWifi;
use esp_idf_svc::wifi::{AuthMethod, Configuration as OtherConfiguration};
use futures::FutureExt;
use heapless::{String, Vec};
use log::*;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;
const SSID: &str = env!("RUST_ESP32_STD_DEMO_WIFI_SSID");
const PASS: &str = env!("RUST_ESP32_STD_DEMO_WIFI_PASS");
fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    //bunch of other initializations
    let peripherals = Peripherals::take().unwrap();
    let syslog = EspSystemEventLoop::take().unwrap();
    let timer_service = EspTaskTimerService::new().unwrap();
    let _wifi = wifi(
        peripherals.modem,
        syslog,
        Some(EspDefaultNvsPartition::take().unwrap()),
        timer_service.clone(),
    )
    .unwrap();
    let mut mutex_wifi = Arc::new(Mutex::new(_wifi));
    info!("Hello, world!");
    let mut server = EspHttpServer::new(&Configuration::default()).unwrap();
    server
        .fn_handler("/", Method::Get, |request| {
            let resp = scan_wifis(Arc::clone(&mutex_wifi));
            request
                .into_ok_response()?
                .write_all(format!("<html><body>Hello world!<br>{}</body></html>", resp).as_bytes())
        })
        .unwrap();

    loop {
        sleep(Duration::from_secs(1));
    }
}
fn scan_wifis(wifi: Arc<std::sync::Mutex<AsyncWifi<EspWifi<'_>>>>) -> std::string::String {
    //AsyncWifi<EspWifi<'static>>
    let mut wifi = wifi.lock().unwrap();
    if !(*wifi).is_started().unwrap() {
        futures::executor::block_on((*wifi).start()).unwrap();
    }
    let mut result: std::string::String = std::string::String::new();
    for item in futures::executor::block_on((*wifi).scan()).unwrap() {
        result = result + &format!("{:?}<br>", item.ssid);
    }
    result
}
use esp_idf_svc::hal::peripheral::Peripheral;
use esp_idf_svc::nvs::EspNvsPartition;
use esp_idf_svc::nvs::NvsDefault;
use esp_idf_svc::timer::{EspTimerService, Task};
use esp_idf_svc::wifi::EspWifi;
pub fn wifi(
    modem: impl Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,
    nvs: Option<EspNvsPartition<NvsDefault>>,
    timer_service: EspTimerService<Task>,
) -> Result<AsyncWifi<EspWifi<'static>>, anyhow::Error> {
    use futures::executor::block_on;

    let mut wifi = AsyncWifi::wrap(
        EspWifi::new(modem, sysloop.clone(), nvs)?,
        sysloop,
        timer_service.clone(),
    )?;
    println!("{}", wifi.is_connected().unwrap());
    block_on(connect_wifi(&mut wifi))?;
    println!("Now The WIFI Is connected");
    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    println!("Wifi DHCP info: {:?}", ip_info);
    println!("{}", wifi.is_connected().unwrap());
    EspPing::default().ping(
        ip_info.subnet.gateway,
        &esp_idf_svc::ping::Configuration::default(),
    )?;
    Ok(wifi)
}

async fn connect_wifi(wifi: &mut AsyncWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    let mut dumy = Vec::<u8, 32>::new();
    dumy.extend_from_slice(SSID.as_bytes());
    let mut dumy1 = Vec::<u8, 64>::new();
    dumy1.extend_from_slice(PASS.as_bytes());

    let wifi_configuration: OtherConfiguration = OtherConfiguration::Client(ClientConfiguration {
        ssid: String::from_utf8(dumy).unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: String::from_utf8(dumy1).unwrap(),
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start().await?;
    info!("Wifi started");

    wifi.connect().await?;
    info!("Wifi connected");

    wifi.wait_netif_up().await?;
    info!("Wifi netif up");

    Ok(())
}

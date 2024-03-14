#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_time::Timer;
use embassy_usb::class::hid::{HidReaderWriter, ReportId, RequestHandler, State};
use embassy_usb::control::OutResponse;
use embassy_usb::{Builder, Config, Handler};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};
use {defmt_rtt as _, panic_probe as _};

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("Get report for {:?}", id);
        None
    }

    fn set_report(&self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {:?}: {=[u8]}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&self, id: Option<ReportId>, dur: u32) {
        info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&self, id: Option<ReportId>) -> Option<u32> {
        info!("Get idle rate for {:?}", id);
        None
    }
}

struct MyDeviceHandler {
    configured: AtomicBool,
}

impl MyDeviceHandler {
    fn new() -> Self {
        MyDeviceHandler {
            configured: AtomicBool::new(false),
        }
    }
}

impl Handler for MyDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        self.configured.store(false, Ordering::Relaxed);
        if enabled {
            info!("Device enabled");
        } else {
            info!("Device disabled");
        }
    }

    fn reset(&mut self) {
        self.configured.store(false, Ordering::Relaxed);
        info!("Bus reset, the Vbus current limit is 100mA");
    }

    fn addressed(&mut self, addr: u8) {
        self.configured.store(false, Ordering::Relaxed);
        info!("USB address set to: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        self.configured.store(configured, Ordering::Relaxed);
        if configured {
            info!("Device configured, it may now draw up to the configured current limit from Vbus.")
        } else {
            info!("Device is no longer configured, the Vbus current limit is 100mA.");
        }
    }
}

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let pico = embassy_rp::init(Default::default());

    // Create the HAL driver
    let driver = Driver::new(pico.USB, Irqs);

    // Create embassy-usb config
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Feral Fantasies");
    config.product = Some("Pico HID Macro Keyboard");
    config.serial_number = Some("1001001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Create embassy-usb DeviceBuider using driver and config
    // Buffers required for building the descriptors
    let mut device_descriptor = [0; 256];
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    // Add Microsoft OS Descriptor
    let mut msos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let request_handler = MyRequestHandler {};
    let mut device_handler = MyDeviceHandler::new();

    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut device_descriptor,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    builder.handler(&mut device_handler);

    // Create builder classes
    let config = embassy_usb::class::hid::Config {
        report_descriptor: KeyboardReport::desc(),
        request_handler: Some(&request_handler),
        poll_ms: 60,
        max_packet_size: 64,
    };
    let hid = HidReaderWriter::<_, 1, 8>::new(&mut builder, &mut state, config);

    // build builder
    let mut usb = builder.build();

    // Run USB device
    let usb_fut = usb.run();

    // Set signal pin for keyboard trigger <--------------- Check this
    // let mut signal_pin = Input::new(pico.PIN_10, Pull::None);

    // Enable schmitt trigger to slightly debounce
    // signal_pin.set_schmitt(true);

    let (reader, mut writer) = hid.split();

    // End of class setup

    let mut led_main = Output::new(pico.PIN_25, Level::Low);
    let mut led_1 = Output::new(pico.PIN_18, Level::Low);
    let mut led_2 = Output::new(pico.PIN_17, Level::Low);
    let mut led_3 = Output::new(pico.PIN_16, Level::Low);
    let mut led_4 = Output::new(pico.PIN_15, Level::Low);

    let button_1 = Input::new(pico.PIN_13, Pull::Up);
    let button_2 = Input::new(pico.PIN_12, Pull::Up);
    let button_3 = Input::new(pico.PIN_11, Pull::Up);
    let button_4 = Input::new(pico.PIN_10, Pull::Up);

    // let in_fut = async {
    //     info!("Waiting for HIGH on pin 10");
    //     signal_pin.wait_for_high().await;
    //     info!("HIGH DETECTED!");
    //     let report = KeyboardReport {
    //         keycodes: [4, 0, 0, 0, 0, 0],
    //         leds: 0,
    //         modifier: 0,
    //         reserved: 0,
    //     };
    //     match writer.write_serialize(&report).await {
    //         Ok(()) => {}
    //         Err(e) => warn!("Failed to send report: {:?}", e),
    //     };
    //     signal_pin.wait_for_low().await;
    //     info!("LOW DETECTED");
    //     let report = KeyboardReport {
    //         keycodes: [0, 0, 0, 0, 0, 0],
    //         leds: 0,
    //         modifier: 0,
    //         reserved: 0,
    //     };
    //     match writer.write_serialize(&report).await {
    //         Ok(()) => {}
    //         Err(e) => warn!("Failed to send report: {:?}", e),
    //     };
    // };

    // let out_fut = async {
    //     reader.run(false, &request_handler).await;
    // };

    // join(usb_fut, join(in_fut, out_fut)).await;

    loop {
        if button_1.is_high() {
            info!("led_1 on!");
            led_1.set_high();
            // let report = KeyboardReport {
            //     keycodes: [4, 0, 0, 0, 0, 0],
            //     leds: 0,
            //     modifier: 0,
            //     reserved: 0,
            // };
            // match writer.write_serialize(&report).await {
            //     Ok(()) => {}
            //     Err(e) => warn!("Failed to send report: {:?}", e),
            // };
        } else {
            led_1.set_low();
        }

        if button_2.is_high() {
            info!("led_2 on!");
            led_2.set_high();
        } else {
            led_2.set_low();
        }

        if button_3.is_high() {
            info!("led_3 on!");
            led_3.set_high();
        } else {
            led_3.set_low();
        }

        if button_4.is_high() {
            info!("led_4 on!");
            led_4.set_high();
        } else {
            led_4.set_low();
        }
    }
}

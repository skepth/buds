// This example showcases how to configure ESP32 timers and the interrupts
// using the TimerDriver API.

use esp_idf_svc::hal::{gpio::Gpio1, peripherals::Peripherals, timer::TimerDriver};
use std::time::Duration;

use esp_idf_svc::hal::gpio::{Output, PinDriver};

use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;

static READING: AtomicI32 = AtomicI32::new(0);

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let mut timer_driver = TimerDriver::new(
        peripherals.timer00,
        &esp_idf_svc::hal::timer::config::Config {
            divider: 1600,
            xtal: false,
            auto_reload: true,
        },
    )
    .unwrap();

    // Now we set the alarm.
    // Since the clock is 80 MHz and the 1600 divider gives us 50 kHz frequency.
    // This means 50,000 ticks per second.
    // If we want the alarm to ring every 10 sec then it should be set to 500,000.
    let ticks_per_second: u64 = 50000;
    timer_driver.set_alarm(10 * ticks_per_second).unwrap();

    let mut led = PinDriver::output(peripherals.pins.gpio1).unwrap();

    // A simple Interrupt Service Routine that toggles an led
    // based on a timer interrupt every 10 sec.
    let blinky_isr = || {
        // led.toggle();
        READING.fetch_add(1, Ordering::Relaxed);
        move |mut led: PinDriver<Gpio1, Output>| led.toggle();
    };

    // The TimeDriver only seems to take closures and with closures, passing led
    // only works with moves. And moves does not cause the toggle to work.
    // Note that the ISR does get called!
    let _ = unsafe { timer_driver.subscribe_nonstatic(blinky_isr).unwrap() };
    timer_driver.set_counter(0).unwrap();
    timer_driver.enable_interrupt().unwrap();
    timer_driver.enable_alarm(true).unwrap();
    timer_driver.enable(true).unwrap();

    log::info!("Running test...");

    loop {
        timer_driver
            .counter()
            .inspect(|x| log::info!("Counter Value: {x}"));
        log::info!("READING: {}", READING.load(Ordering::Relaxed));
        thread::sleep(Duration::from_millis(1000));
    }
}

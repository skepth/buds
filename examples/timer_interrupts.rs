// This example showcases how to configure ESP32 timers and use them
// to trigger interrupt service routines (ISR's).

use std::{error::Error, os::raw::c_void};

use esp_idf_svc::{
    hal::{gpio::Gpio1, peripherals::Peripherals},
    sys::{
        soc_periph_tg_clk_src_legacy_t_TIMER_SRC_CLK_APB, timer_alarm_t_TIMER_ALARM_EN,
        timer_autoreload_t_TIMER_AUTORELOAD_EN, timer_config_t, timer_count_dir_t_TIMER_COUNT_UP,
        timer_enable_intr, timer_group_t_TIMER_GROUP_0, timer_idx_t_TIMER_0, timer_init,
        timer_intr_mode_t_TIMER_INTR_LEVEL, timer_isr_callback_add, timer_set_alarm_value,
        timer_set_counter_value, timer_start, timer_start_t_TIMER_PAUSE, ESP_OK,
    },
};
use std::time::Duration;

use esp_idf_svc::hal::gpio::{Output, PinDriver};

use std::thread;

// A simple Interrupt Service Routine that toggles an led
// based on a timer interrupt every 10 sec.
// An interrupt function should return bool to indicate yield?
#[no_mangle]
extern "C" fn blinker_isr(args: *mut c_void) -> bool {
    // https://stackoverflow.com/questions/24191249/working-with-c-void-in-an-ffi
    let led: &mut PinDriver<Gpio1, Output> =
        unsafe { &mut *(args as *mut PinDriver<Gpio1, Output>) };
    let _ = led.toggle();

    true
}

// Initialize the timer configuration.
fn timer_initialize(
    group_number: u32,
    timer_number: u32,
    timer_config: timer_config_t,
) -> Result<(), Box<dyn Error>> {
    // SAFETY: timer_init() is an ESP32 ABI call.
    let result = unsafe {
        timer_init(
            group_number,
            timer_number,
            &timer_config as *const timer_config_t,
        )
    };
    if result != ESP_OK {
        return Err(format!("Failed to initialize timer.\nReturned: {}", result).into());
    };

    // SAFETY: timer_set_counter_value() is an ESP32 ABI call.
    unsafe { timer_set_counter_value(group_number, timer_number, 0) };

    // Now we set the alarm.
    // Since the clock is 80 MHz and the 1600 divider gives us 50 kHz frequency.
    // This means 50,000 ticks per second.
    // If we want the alarm to ring every 10 sec then it should be set to 500,000.
    let ticks_per_second: u64 = 50000;
    unsafe { timer_set_alarm_value(group_number, timer_number, 10 * ticks_per_second) };

    // Now we enable interrups on this timer?
    unsafe { timer_enable_intr(group_number, timer_number) };

    return Ok(());
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let config = timer_config_t {
        alarm_en: timer_alarm_t_TIMER_ALARM_EN,
        counter_en: timer_start_t_TIMER_PAUSE,
        intr_type: timer_intr_mode_t_TIMER_INTR_LEVEL, // Interrupt triggered as level edge?
        counter_dir: timer_count_dir_t_TIMER_COUNT_UP,
        auto_reload: timer_autoreload_t_TIMER_AUTORELOAD_EN,
        clk_src: soc_periph_tg_clk_src_legacy_t_TIMER_SRC_CLK_APB,
        divider: 1600, // 50 kHz
    };

    let group_number = timer_group_t_TIMER_GROUP_0;
    let timer_number = timer_idx_t_TIMER_0;
    let _ = timer_initialize(group_number, timer_number, config)
        .inspect_err(|e| log::error!("Error: {e}"));

    // Now we setup the callback for the interrupt.
    let mut led = PinDriver::output(peripherals.pins.gpio1).unwrap();
    unsafe {
        timer_isr_callback_add(
            group_number,
            timer_number,
            Some(blinker_isr),
            &mut led as *mut _ as *mut c_void,
            0,
        )
    };

    let _ = unsafe { timer_start(group_number, timer_number) };
    log::info!("Running test...");

    loop {
        thread::sleep(Duration::from_millis(1000));
    }
}

// This example showcases how to read data from a rotary encoder.

use std::{
    error::Error,
    os::raw::c_void,
    sync::atomic::{AtomicI8, Ordering},
    thread,
    time::Duration,
};

use esp_idf_svc::{
    hal::{
        gpio::{Gpio0, Gpio1, Gpio4, Input, Level, Output, PinDriver},
        peripherals::Peripherals,
    },
    sys::{
        soc_periph_tg_clk_src_legacy_t_TIMER_SRC_CLK_APB, timer_alarm_t_TIMER_ALARM_EN,
        timer_autoreload_t_TIMER_AUTORELOAD_EN, timer_config_t, timer_count_dir_t_TIMER_COUNT_UP,
        timer_enable_intr, timer_group_t_TIMER_GROUP_0, timer_idx_t_TIMER_0, timer_init,
        timer_intr_mode_t_TIMER_INTR_LEVEL, timer_isr_callback_add, timer_set_alarm_value,
        timer_set_counter_value, timer_start, timer_start_t_TIMER_PAUSE, ESP_OK,
    },
};

// Global Variable to keep state of the previous reading.
static PREVIOUS_READING: AtomicI8 = AtomicI8::new(0);
static DIRECTION: AtomicI8 = AtomicI8::new(-1);
static TEST: AtomicI8 = AtomicI8::new(0);

// Enum to represent the direction of rotation of the rotaty encoder.
enum EncoderDirection {
    None,
    Clockwise,
    AntiClockwise,
}

// Converts input levels into grey code.
fn convert_to_greycode(input_a: Level, input_b: Level) -> i8 {
    match (input_a, input_b) {
        (Level::Low, Level::Low) => 0,   // (0, 0)
        (Level::Low, Level::High) => 1,  // (0, 1)
        (Level::High, Level::High) => 2, // (1, 1)
        (Level::High, Level::Low) => 3,  // (1, 0)
    }
}

// Determine the direction of rotation.
fn get_rotation_direction(new_reading: i8) -> EncoderDirection {
    // Swap uses atomics to set the PREVIOUS_READING to new_reading
    // while alsi returning the old value set.
    // We are using Sequencially Consistent ordering since the order of reads
    // is important for continuous tracking of direction.
    // https://doc.rust-lang.org/nomicon/atomics.html#sequentially-consistent
    let old_reading = PREVIOUS_READING.swap(new_reading, Ordering::SeqCst);

    match old_reading - new_reading {
        -1 | 3 => EncoderDirection::Clockwise,
        1 | -3 => EncoderDirection::AntiClockwise,
        _ => EncoderDirection::None,
    }
}

// Rotary Encoder Inputs
struct GpioHandle<'a> {
    input_a: PinDriver<'a, Gpio0, Input>,
    input_b: PinDriver<'a, Gpio1, Input>,
    output: PinDriver<'a, Gpio4, Output>,
}

// Interrupt Service Routine to measure direction.
// A simple Interrupt Service Routine that reads the rotary encoder
// based on a timer interrupt 10 times per sec.
#[no_mangle]
extern "C" fn read_rotary_encoder_isr(args: *mut c_void) -> bool {
    // https://stackoverflow.com/questions/24191249/working-with-c-void-in-an-ffi
    let pins: &mut GpioHandle = unsafe { &mut *(args as *mut GpioHandle) };

    // Read the encoder values.
    let grey_code = convert_to_greycode(pins.input_a.get_level(), pins.input_b.get_level());

    // Determine the direction of rotation if any.
    let dir = get_rotation_direction(grey_code);

    match dir {
        EncoderDirection::Clockwise => {
            DIRECTION.store(0, Ordering::SeqCst);
            let _ = TEST.fetch_add(1, Ordering::SeqCst);
            pins.output.set_high();
        }
        EncoderDirection::AntiClockwise => {
            DIRECTION.store(1, Ordering::SeqCst);
            let _ = TEST.fetch_add(-1, Ordering::SeqCst);
        }
        EncoderDirection::None => {
            DIRECTION.store(-1, Ordering::SeqCst);
            pins.output.set_low();
        }
    }
    true
}

// Initialize timer.
fn timer_initialize(group_number: u32, timer_number: u32) -> Result<(), Box<dyn Error>> {
    let timer_config = timer_config_t {
        alarm_en: timer_alarm_t_TIMER_ALARM_EN,
        counter_en: timer_start_t_TIMER_PAUSE,
        intr_type: timer_intr_mode_t_TIMER_INTR_LEVEL, // Interrupt triggered as level edge?
        counter_dir: timer_count_dir_t_TIMER_COUNT_UP,
        auto_reload: timer_autoreload_t_TIMER_AUTORELOAD_EN,
        clk_src: soc_periph_tg_clk_src_legacy_t_TIMER_SRC_CLK_APB,
        divider: 20, // 4 MHz (4 million times per second)
    };
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
    // Since the clock is 80 MHz and the 20 divider gives us 4 MHz frequency.
    // This means 4,000,000 ticks per second.
    // If we want the alarm to ring 50 times every second (0.02 sec), then it
    // should be set to 4,000,000 * 0.02.
    unsafe { timer_set_alarm_value(group_number, timer_number, 80000) };

    // Now we enable interrups on this timer.
    unsafe { timer_enable_intr(group_number, timer_number) };

    return Ok(());
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let mut input_a = PinDriver::input(peripherals.pins.gpio0).unwrap();
    let mut input_b = PinDriver::input(peripherals.pins.gpio1).unwrap();
    let mut output = PinDriver::output(peripherals.pins.gpio4).unwrap();

    let group_number = timer_group_t_TIMER_GROUP_0;
    let timer_number = timer_idx_t_TIMER_0;

    let _ = timer_initialize(group_number, timer_number).inspect_err(|e| log::error!("Error: {e}"));

    let mut handle = GpioHandle {
        input_a,
        input_b,
        output,
    };
    unsafe {
        timer_isr_callback_add(
            group_number,
            timer_number,
            Some(read_rotary_encoder_isr),
            &mut handle as *mut _ as *mut c_void,
            0,
        )
    };
    let _ = unsafe { timer_start(group_number, timer_number) };

    log::info!("Running test...");

    loop {
        log::info!("TEST: {}", TEST.load(Ordering::SeqCst));

        thread::sleep(Duration::from_millis(1000));
    }
}

use crate::{println, sbi::shutdown};
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let msg = info.message().unwrap();
    if let Some(location) = info.location() {
        println!(
            "Panicked at {}:{} {}",
            location.file(),
            location.line(),
            msg
        );
    } else {
        println!("Panicked: {}", msg);
    }

    shutdown()
}

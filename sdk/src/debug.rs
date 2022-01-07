use crate::sys;

pub fn log(msg: String) {
    unsafe {
        sys::debug::log(msg.as_ptr(), msg.len() as u32).unwrap();
    }
}

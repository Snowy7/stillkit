//! Opt-in desktop services sharing the Wayland poll set.
pub(super) mod bus;
mod tray;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
pub(crate) use tray::Tray;
thread_local! { static BUSES: RefCell<Vec<Weak<RefCell<bus::Bus>>>> = const { RefCell::new(Vec::new()) }; }
fn register(bus: &Rc<RefCell<bus::Bus>>) {
    BUSES.with_borrow_mut(|b| b.push(Rc::downgrade(bus)));
}
pub(super) fn poll_fds() -> Vec<libc::pollfd> {
    BUSES.with_borrow_mut(|buses| {
        buses.retain(|b| b.strong_count() > 0);
        buses
            .iter()
            .filter_map(Weak::upgrade)
            .filter_map(|b| b.borrow().poll_fd())
            .collect()
    })
}
pub(super) fn dispatch() -> bool {
    BUSES.with_borrow(|buses| {
        let mut ready = false;
        for bus in buses.iter().filter_map(Weak::upgrade) {
            ready |= bus.borrow_mut().dispatch();
        }
        ready
    })
}

mod hotkey;
pub(super) mod portal;
pub(crate) use hotkey::Hotkey;



mod wire;

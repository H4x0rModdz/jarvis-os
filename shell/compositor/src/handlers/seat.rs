use crate::state::JarvisCompositor;
use smithay::{
    delegate_seat,
    input::{pointer::CursorImageStatus, Seat, SeatHandler, SeatState},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::Serial,
};

impl SeatHandler for JarvisCompositor {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&Self::KeyboardFocus>) {
        seat.get_keyboard()
            .unwrap()
            .set_focus(self, focused.cloned(), Serial::from(0));
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
    }
}

delegate_seat!(JarvisCompositor);

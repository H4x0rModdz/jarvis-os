use crate::state::JarvisCompositor;
use smithay::{
    delegate_seat,
    input::{
        keyboard::{KeyboardTarget, KeysymHandle, ModifiersState},
        pointer::{
            AxisFrame, ButtonEvent, CursorImageStatus, MotionEvent, PointerTarget,
            RelativeMotionEvent,
        },
        Seat, SeatHandler, SeatState,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{IsAlive, Serial},
    wayland::seat::WaylandFocus,
};

impl SeatHandler for JarvisCompositor {
    type KeyboardFocus = smithay::desktop::Window;
    type PointerFocus = smithay::desktop::Window;
    type TouchFocus = smithay::desktop::Window;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&Self::KeyboardFocus>) {
        let dh = &self.display_handle;
        let client = focused
            .and_then(|w| w.toplevel())
            .and_then(|t| dh.get_client(t.wl_surface().id()).ok());

        // Update keyboard focus
        seat.get_keyboard()
            .unwrap()
            .set_focus(self, focused.cloned(), Serial::now());
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
    }
}

delegate_seat!(JarvisCompositor);

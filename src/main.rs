use std::{future::Future, os::fd::AsFd};

use wayland_client::{
    event_created_child,
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_registry,
    Connection, Dispatch, Proxy,
};

use nix::poll::{PollFd, PollFlags, PollTimeout};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1,
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

#[derive(Debug)]
struct BaseState;

// so interesting, it is just need to invoke once, it just used to get the globals
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for BaseState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

#[allow(unused)]
#[derive(Debug, Default)]
struct SecondState {
    running: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for SecondState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1, ()> for SecondState {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        event: <zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            println!("it is {:?}", toplevel)
        }
    }

    event_created_child!(SecondState, zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, ()> for SecondState {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
        event: <zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        println!("event is {:?}", event);
    }
}

#[derive(Debug, Clone, Copy)]
struct ConnectionPoll<'a> {
    connection: &'a Connection,
    fd: PollFd<'a>,
}

impl<'a> ConnectionPoll<'a> {
    fn new(connection: &'a Connection) -> Self {
        ConnectionPoll {
            connection,
            fd: PollFd::new(connection.as_fd(), PollFlags::POLLIN),
        }
    }
}

use std::task::Poll;
impl<'a> Future for ConnectionPoll<'a> {
    type Output = ();
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.fd.events().contains(PollFlags::POLLIN) {
            self.connection.flush().ok();
            Poll::Ready(())
        } else {
            nix::poll::poll(&mut [self.fd], PollTimeout::NONE).ok();
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[tokio::main]
async fn main() {
    let connection = Connection::connect_to_env().unwrap();
    let (globals, _) = registry_queue_init::<BaseState>(&connection).unwrap(); // We just need the
                                                                               // global, the
                                                                               // event_queue is
                                                                               // not needed, we
                                                                               // do not need
                                                                               // BaseState after
                                                                               // this anymore

    let mut state = SecondState::default();

    let mut event_queue = connection.new_event_queue::<SecondState>();
    let qh = event_queue.handle();

    // get WlCompositor

    globals
        .bind::<ZwlrForeignToplevelManagerV1, _, _>(&qh, 1..=3, ())
        .unwrap();

    let poll_connection = ConnectionPoll::new(&connection);

    loop {
        poll_connection.await;
        event_queue.roundtrip(&mut state).unwrap();
    }
}

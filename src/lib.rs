mod backoff;
mod connection;
pub use connection::StreamError as Error;
mod messages;
mod presence;
mod register;
pub use presence::*;

use backoff::Backoff;
use connection::Connection;
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    cell::RefCell,
    process,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_IO_THREAD_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
enum Event {
    Connected(Option<User>),
    Disconnected(Option<Error>),
    GotError(Error),
    GameJoined(String),
    StartedSpectating(String),
    JoinRequested(User),
}

#[derive(Default)]
pub struct EventHandlers {
    pub connect: Option<Box<dyn FnMut(Option<User>)>>,
    pub disconnect: Option<Box<dyn FnMut(Option<Error>)>>,
    pub error: Option<Box<dyn FnMut(Error)>>,
    pub join_game: Option<Box<dyn FnMut(String)>>,
    pub spectate_game: Option<Box<dyn FnMut(String)>>,
    pub join_request: Option<Box<dyn FnMut(User)>>,
}

struct Nonce(i32);

impl Nonce {
    fn next(&mut self) -> i32 {
        let result = self.0;
        self.0 = self.0.wrapping_add(1);
        result
    }
}

pub struct Rpc {
    shared_state: Arc<SharedState>,
    message_tx: Sender<Vec<u8>>,
    event_rx: Receiver<Event>,
    io_thread: Option<JoinHandle<()>>,

    handlers: EventHandlers,
    pid: u32,
    nonce: Nonce,
}

struct SharedState {
    presence: Mutex<Vec<u8>>,
    presence_updated: AtomicBool,
    is_connected: AtomicBool,
    stopped: AtomicBool,
}

impl Rpc {
    pub fn new(app_id: String, handlers: EventHandlers, auto_register: bool) -> Self {
        #[cfg(target_os = "macos")] // TODO: Support other OSes too
        if auto_register {
            let _ = register::register_url(&app_id);
        }

        let (message_tx, message_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();

        let shared_state = Arc::new(SharedState {
            presence: Mutex::new(Vec::new()),
            presence_updated: AtomicBool::new(false),
            is_connected: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        });

        let shared_state_clone = Arc::clone(&shared_state);
        let io_thread = thread::Builder::new()
            .name("Discord RPC".to_string())
            .spawn(move || run_io_thread(app_id, message_rx, event_tx, shared_state_clone))
            .expect("Couldn't spawn Discord RPC IO thread");

        Rpc {
            shared_state,
            message_tx,
            event_rx,
            io_thread: Some(io_thread),

            handlers,
            pid: process::id(),
            nonce: Nonce(1),
        }
    }

    fn send_message<T: Serialize>(&self, message: &T) -> serde_json::Result<()> {
        let _ = self.message_tx.send(serde_json::to_vec(message)?);
        self.io_thread.as_ref().unwrap().thread().unpark();
        Ok(())
    }

    fn toggle_event_subscription<const ENABLED: bool>(&mut self, event: &str) {
        let nonce = self.nonce.next();
        let _ = self.send_message(&messages::ToggleSubscription::<ENABLED> { nonce, event });
    }

    pub fn modify_handlers(&mut self, f: impl FnOnce(&mut EventHandlers)) {
        let had_join_game_handler = self.handlers.join_game.is_some();
        let had_spectate_game_handler = self.handlers.spectate_game.is_some();
        let had_join_request_handler = self.handlers.join_request.is_some();
        f(&mut self.handlers);
        macro_rules! toggle_event_subscription {
            ($prev: expr, $new: expr, $name: expr) => {
                match ($prev, $new) {
                    (false, true) => self.toggle_event_subscription::<true>($name),
                    (true, false) => self.toggle_event_subscription::<false>($name),
                    _ => {}
                }
            };
        }
        toggle_event_subscription!(
            had_join_game_handler,
            self.handlers.join_game.is_some(),
            "ACTIVITY_JOIN"
        );
        toggle_event_subscription!(
            had_spectate_game_handler,
            self.handlers.spectate_game.is_some(),
            "ACTIVITY_SPECTATE"
        );
        toggle_event_subscription!(
            had_join_request_handler,
            self.handlers.join_request.is_some(),
            "ACTIVITY_JOIN_REQUEST"
        );
    }

    pub fn update_presence(&mut self, presence: Option<&Presence>) {
        {
            let mut presence_raw = self.shared_state.presence.lock();
            presence_raw.clear();
            let _ = serde_json::to_writer(
                &mut *presence_raw,
                &messages::SetActivity {
                    pid: self.pid,
                    nonce: self.nonce.next(),
                    presence,
                },
            );
        }
        self.shared_state
            .presence_updated
            .store(true, Ordering::Release);
        self.io_thread.as_ref().unwrap().thread().unpark();
    }

    pub fn reply_to_join_request(&mut self, user_id: &str, accepted: bool) {
        if !self.shared_state.is_connected.load(Ordering::Relaxed) {
            return;
        }
        let nonce = self.nonce.next();
        let _ = self.send_message(&messages::JoinReply {
            user_id,
            accepted,
            nonce,
        });
    }

    pub fn check_events(&mut self) {
        macro_rules! run_cb {
            ($callback: expr, $($args: tt)*) => {
                if let Some(callback) = &mut $callback {
                    callback($($args)*);
                }
            }
        }
        for event in self.event_rx.try_iter() {
            match event {
                Event::Connected(user) => run_cb!(self.handlers.connect, user),
                Event::Disconnected(err) => run_cb!(self.handlers.disconnect, err),
                Event::GotError(err) => run_cb!(self.handlers.error, err),
                Event::GameJoined(secret) => run_cb!(self.handlers.join_game, secret),
                Event::StartedSpectating(secret) => run_cb!(self.handlers.spectate_game, secret),
                Event::JoinRequested(user) => run_cb!(self.handlers.join_request, user),
            }
        }
    }
}

impl Drop for Rpc {
    fn drop(&mut self) {
        self.shared_state
            .presence_updated
            .store(false, Ordering::Relaxed);
        self.shared_state.stopped.store(true, Ordering::Relaxed);
        if let Some(thread) = self.io_thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

struct ReconnectionTime {
    backoff: Backoff,
    next_time: Instant,
}

impl ReconnectionTime {
    fn new() -> Self {
        ReconnectionTime {
            backoff: Backoff::new(Duration::from_millis(500), Duration::from_secs(60)),
            next_time: Instant::now(),
        }
    }

    fn calc_next(&mut self) {
        let delay = self.backoff.next();
        self.next_time = Instant::now() + delay;
    }
}

fn run_io_thread(
    app_id: String,
    message_rx: Receiver<Vec<u8>>,
    event_tx: Sender<Event>,
    shared_state: Arc<SharedState>,
) {
    let mut connection = Connection::new(app_id);
    let reconnection_time = Rc::new(RefCell::new(ReconnectionTime::new()));
    
    {
        let event_tx = event_tx.clone();
        let reconnection_time = Rc::clone(&reconnection_time);
        connection.on_connect = Some(Box::new(move |user| {
            event_tx.send(Event::Connected(user)).unwrap();
            reconnection_time.borrow_mut().backoff.reset();
        }));
    }

    {
        let event_tx = event_tx.clone();
        let reconnection_time = Rc::clone(&reconnection_time);
        connection.on_disconnect = Some(Box::new(move |err| {
            event_tx.send(Event::Disconnected(err.cloned())).unwrap();
            reconnection_time.borrow_mut().calc_next();
        }));
    }

    while !shared_state.stopped.load(Ordering::Relaxed) {
        if connection.is_connected() {
            while let Ok(Some(mut message)) = connection.read_json::<messages::Event>() {
                match message.event.as_str() {
                    "ERROR" => {
                        if let Ok(err) = serde_json::from_value::<Error>(message.data.into()) {
                            let _ = event_tx.send(Event::GotError(err));
                        }
                    }

                    "ACTIVITY_JOIN" => {
                        if let Some(secret) = message
                            .data
                            .get("secret")
                            .and_then(|secret| secret.as_str())
                        {
                            let _ = event_tx.send(Event::GameJoined(secret.to_string()));
                        }
                    }

                    "ACTIVITY_SPECTATE" => {
                        if let Some(secret) = message
                            .data
                            .get("secret")
                            .and_then(|secret| secret.as_str())
                        {
                            let _ = event_tx.send(Event::StartedSpectating(secret.to_string()));
                        }
                    }

                    "ACTIVITY_JOIN_REQUEST" => {
                        if let Some(user) = message
                            .data
                            .remove("user")
                            .and_then(|user| serde_json::from_value::<User>(user).ok())
                        {
                            let _ = event_tx.send(Event::JoinRequested(user));
                        }
                    }

                    _ => {}
                }
            }

            if shared_state
                .presence_updated
                .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                let _ = connection.write_raw(&shared_state.presence.lock()[..]);
            }

            for message in message_rx.try_iter() {
                let _ = connection.write_raw(&message);
            }
        } else {
            let mut reconnection_time = reconnection_time.borrow_mut();
            if Instant::now() >= reconnection_time.next_time {
                reconnection_time.calc_next();
                drop(reconnection_time);
                let _ = connection.open();
            }
        }

        shared_state
            .is_connected
            .store(connection.is_connected(), Ordering::Relaxed);
        thread::park_timeout(MAX_IO_THREAD_TIMEOUT);
    }
}

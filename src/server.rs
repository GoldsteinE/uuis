use std::{
    convert::Infallible,
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, ToSocketAddrs},
    os::unix::net::UnixListener,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
};

use color_eyre::eyre::{self, bail, eyre};
use crossbeam::channel::{self, Receiver, Sender};
use druid::Target;
use enumflags2::BitFlags;
use parking_lot::Mutex;
use serde::Serialize;

use crate::{
    types::{
        ClientRequest, Event, Registration, ServerEvent, Subscription, CLIENT_REQUEST_SELECTOR,
    },
    ui::{self, UiInitialState},
};

pub const PROTOCOL_VERSION: u8 = 0;

pub struct Server {
    busy: Mutex<()>,
    last_id: AtomicUsize,
}

impl Server {
    fn new() -> Arc<Self> {
        Arc::new(Server {
            busy: Mutex::new(()),
            last_id: AtomicUsize::new(0),
        })
    }

    fn send_message<M: Serialize, W: Write>(mut write: &mut W, message: &M) -> eyre::Result<()> {
        serde_json::to_writer(&mut write, message)?;
        writeln!(write)?;
        Ok(())
    }

    fn send_events<W>(
        self: Arc<Self>,
        events: Receiver<Event>,
        subscription: BitFlags<Subscription>,
        write: Arc<Mutex<W>>,
    ) -> eyre::Result<Infallible>
    where
        W: Write + Send,
    {
        loop {
            let event = events.recv()?;
            if event.needed(subscription) {
                let message = ServerEvent::from(event);
                Self::send_message(&mut *write.lock(), &message)?;
            }
        }
    }

    fn serve_client<R, W>(
        self: Arc<Self>,
        read: R,
        mut write: W,
        client_id: usize,
        ui_sender: Sender<UiInitialState>,
    ) -> eyre::Result<()>
    where
        R: Read,
        W: Write + Send + 'static,
    {
        let mut lines = BufReader::new(read).lines();
        let registration_raw = lines
            .next()
            .ok_or_else(|| eyre!("didn't receive registration"))??;
        let registration: Registration = serde_json::from_str(&registration_raw)?;
        if registration.protocol_version > PROTOCOL_VERSION {
            Self::send_message(&mut write, &ServerEvent::ServerTooOld(PROTOCOL_VERSION))?;
            bail!(
                "server is too old for client {} with protocol version {}",
                client_id,
                registration.protocol_version,
            );
        }

        let _guard = match self.busy.try_lock() {
            Some(guard) => guard,
            None => {
                Self::send_message(&mut write, &ServerEvent::Busy)?;
                self.busy.lock()
            }
        };

        let (sender, receiver) = channel::unbounded();
        let (control_sender, control_receiver) = channel::bounded(1);
        ui_sender.send(UiInitialState {
            events: sender,
            control: control_sender,
            matcher: registration.matcher,
        })?;
        // TODO error handling
        let control = control_receiver.recv().unwrap();
        drop(control_receiver);

        Self::send_message(&mut write, &ServerEvent::Registered(client_id))?;

        let write = Arc::new(Mutex::new(write));
        let events_write = Arc::clone(&write);
        let this = Arc::clone(&self);
        let _events_thread = thread::spawn(move || {
            if let Err(_err) = this.send_events(receiver, registration.subscribe_to, events_write) {
                // TODO logging
            }
        });

        let mut stopped = false;
        for line in lines {
            let line = line?;
            let req: Result<ClientRequest, _> = serde_json::from_str(&line);
            match req {
                Ok(req) => {
                    let stop = matches!(req, ClientRequest::Stop);
                    // TODO error handling
                    control.submit_command(
                        CLIENT_REQUEST_SELECTOR,
                        Box::new(req),
                        Target::Global,
                    )?;
                    if stop {
                        stopped = true;
                        break;
                    }
                }
                Err(err) => {
                    todo!("error handling: {}", err)
                }
            }
        }

        if !stopped {
            control.submit_command(
                CLIENT_REQUEST_SELECTOR,
                Box::new(ClientRequest::Stop),
                Target::Global,
            )?;
        }

        Ok(())
    }

    fn next_id(&self) -> usize {
        self.last_id.fetch_add(1, Ordering::Relaxed)
    }

    fn start_ui(&self) -> Sender<UiInitialState> {
        let (sender, receiver) = channel::bounded(1);
        thread::spawn(move || ui::run_ui(receiver));
        sender
    }

    pub fn run_tcp<A>(addr: A) -> io::Result<Infallible>
    where
        A: ToSocketAddrs,
    {
        let this = Self::new();
        let listener = TcpListener::bind(addr)?;
        let ui_sender = this.start_ui();
        loop {
            // TODO: probably tracing + continue to accept
            let (stream, _addr) = listener.accept()?;
            let this = this.clone();
            let ui_sender = ui_sender.clone();
            thread::spawn(move || {
                // TODO: proper error handling
                let cloned_stream = stream.try_clone().unwrap();
                let client_id = this.next_id();
                if let Err(err) = this.serve_client(stream, cloned_stream, client_id, ui_sender) {
                    // TODO: proper error handling
                    panic!("err in serve_client: {}", err);
                }
            });
        }
    }

    pub fn run_unix<A>(addr: A) -> io::Result<Infallible>
    where
        A: AsRef<Path>,
    {
        let this = Self::new();
        let listener = UnixListener::bind(addr)?;
        let ui_sender = this.start_ui();
        loop {
            // TODO: probably tracing + continue to accept
            let (stream, _addr) = listener.accept()?;
            let this = this.clone();
            let ui_sender = ui_sender.clone();
            thread::spawn(move || {
                // TODO: proper error handling
                let cloned_stream = stream.try_clone().unwrap();
                let client_id = this.next_id();
                if let Err(err) = this.serve_client(stream, cloned_stream, client_id, ui_sender) {
                    // TODO: proper error handling
                    panic!("err in serve_client: {}", err);
                }
            });
        }
    }
}

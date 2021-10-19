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

use color_eyre::eyre::{self, bail, eyre, WrapErr as _};
use crossbeam::channel::{self, Receiver, Sender};
use druid::Target;
use enumflags2::BitFlags;
use parking_lot::Mutex;
use serde::Serialize;

use crate::{
    socket_traits::{Listener, NetStream},
    types::{
        ClientRequest, Event, Registration, ServerEvent, Subscription, CLIENT_REQUEST_SELECTOR,
    },
    ui::self,
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
        events: &Receiver<Event>,
        subscription: BitFlags<Subscription>,
        write: &Mutex<W>,
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
        ui_sender: &Sender<ui::InitialState>,
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

        let _guard = if let Some(guard) = self.busy.try_lock() {
            guard
        } else {
            Self::send_message(&mut write, &ServerEvent::Busy)?;
            self.busy.lock()
        };

        let (sender, receiver) = channel::unbounded();
        let (control_sender, control_receiver) = channel::bounded(1);
        ui_sender.send(ui::InitialState {
            client_id,
            events: sender,
            control: control_sender,
            matcher: registration.matcher,
        })?;

        let control = control_receiver
            .recv()
            .wrap_err("failed to receive ExtEventSink from UI thread")?;
        drop(control_receiver);

        Self::send_message(&mut write, &ServerEvent::Registered(client_id))?;

        let write = Arc::new(Mutex::new(write));
        let events_write = Arc::clone(&write);
        let _events_thread = thread::spawn(move || {
            if let Err(err) =
                Self::send_events(&receiver, registration.subscribe_to, &*events_write)
            {
                tracing::info!(
                    client_id = client_id,
                    "client stopped listening for events: {}",
                    err
                );
            }
        });

        let mut stopped = false;
        for line in lines {
            let line = line?;
            let req: Result<ClientRequest, _> = serde_json::from_str(&line);
            match req {
                Ok(req) => {
                    let stop = matches!(req, ClientRequest::Stop);
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
                    tracing::warn!(
                        line = line.as_str(),
                        "failed to parse client request: {}",
                        err
                    );
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

    fn start_ui() -> Sender<ui::InitialState> {
        let (sender, receiver) = channel::bounded(1);
        thread::spawn(move || ui::run(&receiver));
        sender
    }

    fn run<S: NetStream + Send + 'static, L: Listener<Stream = S>>(
        self: Arc<Self>,
        listener: &L,
    ) -> io::Result<Infallible> {
        let ui_sender = Self::start_ui();
        loop {
            let (stream, _addr) = match listener.accept() {
                Ok(pair) => pair,
                Err(err) => {
                    tracing::error!("failed to accept TCP connection: {}", err);
                    continue;
                }
            };

            let this = Arc::clone(&self);
            let ui_sender = ui_sender.clone();
            thread::spawn(move || {
                let client_id = this.next_id();
                let _span = tracing::info_span!("client-thread", client_id = client_id);

                let cloned_stream = match stream.try_clone() {
                    Ok(cloned) => cloned,
                    Err(err) => {
                        tracing::error!("failed to clone stream: {}", err);
                        return;
                    }
                };
                if let Err(err) = this.serve_client(stream, cloned_stream, client_id, &ui_sender) {
                    tracing::error!("error while serving client: {}", err);
                }
            });
        }
    }

    pub fn run_tcp<A>(addr: A) -> io::Result<Infallible>
    where
        A: ToSocketAddrs,
    {
        Self::new().run(&TcpListener::bind(addr)?)
    }

    pub fn run_unix<A>(addr: A) -> io::Result<Infallible>
    where
        A: AsRef<Path>,
    {
        Self::new().run(&UnixListener::bind(addr)?)
    }
}

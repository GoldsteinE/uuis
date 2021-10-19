use std::fmt::Debug;

use crossbeam::channel::{Receiver, Sender};
use druid::{
    keyboard_types::Key,
    theme,
    widget::{prelude::*, Controller, Flex, Label, List, Painter, TextBox},
    AppLauncher, Command, Data, ExtEventSink, KeyEvent, Lens, Rect, Screen, Selector, Target,
    WidgetExt as _, WindowDesc, WindowHandle,
};

use crate::types::{
    self, Choice, ChoiceSet, ClientRequest, Indices, Matcher, CLIENT_REQUEST_SELECTOR,
};

struct WindowMoved;

static WINDOW_MOVED_SELECTOR: Selector<WindowMoved> = Selector::new("WindowMoved");

#[derive(Debug, Default, Clone, Data, Lens)]
pub struct State {
    matcher: Matcher,
    window_moved: bool,

    input: String,
    elems: ChoiceSet,
}

pub struct TypeWatcher {
    events: Sender<types::Event>,
}

impl TypeWatcher {
    /// Send event to the controlling thread, closing window on error
    fn send_event(&self, window: &WindowHandle, event: types::Event) {
        if self.events.send(event).is_err() {
            tracing::error!("controlling thread stopped listening for events");
            window.close();
        }
    }
}

impl<T> Controller<State, T> for TypeWatcher
where
    T: Widget<State>,
{
    fn event(
        &mut self,
        child: &mut T,
        ctx: &mut EventCtx<'_, '_>,
        event: &Event,
        data: &mut State,
        env: &Env,
    ) {
        // Moving window to the desired position
        ctx.request_focus();
        let old_input = data.input.clone();

        match event {
            Event::KeyDown(KeyEvent {
                key: Key::ArrowUp, ..
            }) => {
                match data.elems.selected {
                    None => data.elems.selected = data.elems.len().checked_sub(1),
                    Some(selected) => data.elems.selected = selected.checked_sub(1),
                }
                ctx.set_handled();
            }
            Event::KeyDown(KeyEvent {
                key: Key::ArrowDown,
                ..
            }) => {
                match data.elems.selected {
                    None if data.elems.is_empty() => {}
                    None => data.elems.selected = Some(0),
                    Some(selected) if selected < data.elems.len().saturating_sub(1) => {
                        data.elems.selected = Some(selected + 1);
                    }
                    Some(_) => {}
                }
                ctx.set_handled();
            }
            Event::KeyDown(KeyEvent {
                key: Key::Enter, ..
            }) => {
                if let Some(selected) = data.elems.selected {
                    if let Some(option) = data.elems.options.iter().nth(selected) {
                        self.send_event(ctx.window(), types::Event::Select(Some(option.id)));
                    } else {
                        tracing::error!(".elems is shorter than implied by selected");
                        data.elems.selected = None;
                    }
                } else {
                    self.send_event(ctx.window(), types::Event::Select(None));
                }
            }
            Event::Command(command) => {
                if let Some(user_request) = command.get(CLIENT_REQUEST_SELECTOR) {
                    match user_request {
                        ClientRequest::Stop => ctx.window().close(),
                        ClientRequest::SetChoices(choices) => {
                            data.elems = choices.clone();
                            if let Some(selected) = data.elems.selected {
                                if selected >= choices.len() {
                                    data.elems.selected = Some(choices.len() - 1);
                                }
                            }

                            if data.matcher == Matcher::Fuzzy {
                                data.elems.fuzzy_sort(&data.input);
                            }
                        }
                        ClientRequest::SetInput(input) => {
                            data.input = input.clone();
                        }
                    }
                }

                if command.get(WINDOW_MOVED_SELECTOR).is_some() {
                    data.window_moved = true;
                }
            }
            _ => {}
        }

        child.event(ctx, event, data, env);

        if data.matcher == Matcher::Fuzzy && old_input != data.input {
            data.elems.fuzzy_sort(&data.input);
            data.elems.selected = Some(0).filter(|_| !data.elems.options.is_empty());
        }
    }

    fn update(
        &mut self,
        child: &mut T,
        ctx: &mut UpdateCtx<'_, '_>,
        old_data: &State,
        data: &State,
        env: &Env,
    ) {
        if old_data.input != data.input {
            self.events
                .send(types::Event::InputChange(data.input.clone()))
                .ok();
        }

        if old_data.elems.selected != data.elems.selected {
            if let Some(selected) = data.elems.selected {
                self.events.send(types::Event::CursorMove(selected)).ok();
            }
        }

        child.update(ctx, old_data, data, env);
    }

    fn lifecycle(
        &mut self,
        child: &mut T,
        ctx: &mut LifeCycleCtx<'_, '_>,
        event: &LifeCycle,
        data: &State,
        env: &Env,
    ) {
        if !data.window_moved {
            if let LifeCycle::Size(Size { width, height }) = event {
                let window = ctx.window();

                let scale = match window.get_scale() {
                    Ok(scale) => scale,
                    Err(err) => {
                        tracing::warn!(
                            "failed to get window scale: {}; can't move the window",
                            err
                        );
                        return;
                    }
                };
                let current_position = window.get_position();
                let mut actually_moved = false;
                for monitor in Screen::get_monitors() {
                    let Rect { x0, y0, x1, y1 } = monitor.virtual_work_rect();
                    let (x0, y0) = scale.px_to_dp_xy(x0, y0);
                    let (x1, y1) = scale.px_to_dp_xy(x1, y1);

                    if (Rect { x0, y0, x1, y1 }.contains(current_position)) {
                        let screen_width = x1 - x0;
                        let screen_height = y1 - y0;
                        window.set_position((
                            // TODO config
                            x0 + screen_width * 0.5 - width * 0.5,
                            y0 + screen_height * 0.3 - height * 0.5,
                        ));
                        actually_moved = true;
                        break;
                    }
                }

                ctx.submit_command(Command::new(
                    WINDOW_MOVED_SELECTOR,
                    WindowMoved,
                    Target::Global,
                ));

                if !actually_moved {
                    tracing::warn!("failed to find monitor containing target window");
                }
            }
        }

        child.lifecycle(ctx, event, data, env);
    }
}

fn root(events: Sender<types::Event>) -> impl Widget<State> {
    Flex::column()
        .with_child(
            TextBox::new()
                .with_placeholder("Query...")
                .with_text_size(32.0)
                .fix_width(512.0)
                .lens(State::input)
                .controller(TypeWatcher { events }),
        )
        .with_child(
            List::new(|| {
                Label::new(|(_, item): &(Indices, Choice), _env: &_| String::from(&*item.text))
                    .with_text_size(32.0)
                    .with_text_alignment(druid::TextAlignment::Start)
                    .fix_width(512.0)
                    .background(Painter::new(
                        move |paint, (idx, _): &(Indices, Choice), env| {
                            let color = if idx.is_selected() {
                                env.get(theme::SELECTED_TEXT_BACKGROUND_COLOR)
                            } else {
                                env.get(theme::WINDOW_BACKGROUND_COLOR)
                            };

                            let shape = paint.size().to_rect();
                            paint.fill(shape, &color);
                        },
                    ))
            })
            .lens(State::elems),
        )
}

#[must_use]
pub fn window(events: Sender<types::Event>) -> WindowDesc<State> {
    WindowDesc::new(root(events))
        .show_titlebar(false)
        .window_size_policy(druid::WindowSizePolicy::Content)
        .resizable(false)
        .title("uuis")
}

pub struct InitialState {
    pub client_id: usize,
    pub events: Sender<types::Event>,
    pub control: Sender<ExtEventSink>,
    pub matcher: Matcher,
}

pub fn run(chan: &Receiver<InitialState>) {
    loop {
        let init = match chan.recv() {
            Ok(init) => init,
            Err(err) => {
                tracing::error!("lost connection to the main thread: {}", err);
                break;
            }
        };

        let _span = tracing::info_span!("ui-iteration", client_id = init.client_id);

        tracing::info!("received request to start UI");
        let window = window(init.events.clone());
        let launcher = AppLauncher::with_window(window);
        let control = launcher.get_external_handle();

        if init.control.send(control).is_err() {
            tracing::error!("failed to send ExtEventSink to the controlling thread");
            continue;
        }

        if let Err(err) = launcher.launch(State {
            matcher: init.matcher,
            ..State::default()
        }) {
            tracing::error!("failed to create a new window: {}", err);
            break;
        }

        tracing::info!("window closed, looping");

        if init.events.send(types::Event::WindowClosed).is_err() {
            tracing::error!("failed to send WindowClosedEvent to the controlling thread");
            continue;
        }
    }
}

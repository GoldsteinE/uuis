use std::fmt::Debug;

use crossbeam::channel::{Receiver, Sender};
use druid::{
    keyboard_types::Key,
    theme,
    widget::{prelude::*, Controller, Flex, Label, List, Painter, TextBox},
    AppLauncher, Data, ExtEventSink, KeyEvent, Lens, WidgetExt as _, WindowDesc,
};

use crate::types::{self, CLIENT_REQUEST_SELECTOR, Choice, ChoiceSet, ClientRequest, Indices, Matcher};

const COUNTER_KEY: druid::Key<u64> = druid::Key::new("uuis.counter");

#[derive(Debug, Default, Clone, Data, Lens)]
pub struct State {
    input: String,
    elems: ChoiceSet,
    matcher: Matcher,
}

pub struct TypeWatcher {
    events: Sender<types::Event>,
}

impl<T> Controller<State, T> for TypeWatcher
where
    T: Widget<State>,
{
    fn update(
        &mut self,
        child: &mut T,
        ctx: &mut UpdateCtx,
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

        child.update(ctx, old_data, data, env)
    }

    fn event(
        &mut self,
        child: &mut T,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut State,
        env: &Env,
    ) {
        ctx.request_focus();
        let old_input = data.input.clone();

        match event {
            Event::KeyDown(KeyEvent {
                key: Key::ArrowUp, ..
            }) => {
                match data.elems.selected {
                    None => data.elems.selected = data.elems.len().checked_sub(1),
                    Some(selected) => data.elems.selected = Some(selected.saturating_sub(1)),
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
                    // TODO error handling
                    if let Some(option) = data.elems.options.iter().nth(selected) {
                        self.events.send(types::Event::Select(option.id)).unwrap();
                    } else {
                        // TODO error handling
                    }
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
                    }
                }
            }
            _ => {}
        }

        child.event(ctx, event, data, env);

        if data.matcher == Matcher::Fuzzy && old_input != data.input {
            data.elems.fuzzy_sort(&data.input);
            data.elems.selected = Some(0).filter(|_| !data.elems.options.is_empty())
        }
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

pub fn window(events: Sender<types::Event>) -> WindowDesc<State> {
    WindowDesc::new(root(events))
        .show_titlebar(false)
        .window_size_policy(druid::WindowSizePolicy::Content)
        .resizable(false)
        .title("uuis")
}

pub struct UiInitialState {
    pub events: Sender<types::Event>,
    pub control: Sender<ExtEventSink>,
    pub matcher: Matcher,
}

pub fn run_ui(chan: Receiver<UiInitialState>) {
    loop {
        let init = chan.recv().unwrap();
        tracing::info!("Received request to start UI");
        let window = window(init.events);
        let launcher = AppLauncher::with_window(window);
        let control = launcher.get_external_handle();
        // TODO error handling
        init.control.send(control).unwrap();
        launcher
            .launch(State {
                matcher: init.matcher,
                ..State::default()
            })
            .unwrap();
        tracing::info!("Window closed, looping");
    }
}

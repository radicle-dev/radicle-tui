use std::collections::HashMap;

use anyhow::Result;

use radicle::cob::issue::{Issue, IssueId};

use tuirealm::{AttrValue, Attribute, Frame, NoUserEvent, State, StateValue, Sub, SubClause};

use radicle_tui as tui;

use tui::cob;
use tui::ui::context::Context;
use tui::ui::layout;
use tui::ui::theme::Theme;
use tui::ui::widget::context::{Progress, Shortcuts};
use tui::ui::widget::Widget;
use tui::ViewPage;

use super::{
    Application, Cid, IssueCid, IssueCobMessage, IssueMessage, ListCid, Message, PopupMessage,
};

use super::subscription;
use super::ui;

///
/// Home
///
pub struct ListPage {
    active_component: ListCid,
    shortcuts: HashMap<ListCid, Widget<Shortcuts>>,
}

impl ListPage {
    pub fn new(theme: Theme) -> Self {
        let shortcuts = Self::build_shortcuts(&theme);
        Self {
            active_component: ListCid::IssueBrowser,
            shortcuts,
        }
    }

    fn activate(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        cid: ListCid,
    ) -> Result<()> {
        self.active_component = cid;
        let cid = Cid::List(self.active_component.clone());
        app.active(&cid)?;
        app.attr(&cid, Attribute::Focus, AttrValue::Flag(true))?;

        Ok(())
    }

    fn build_shortcuts(theme: &Theme) -> HashMap<ListCid, Widget<Shortcuts>> {
        [(
            ListCid::IssueBrowser,
            tui::ui::shortcuts(
                theme,
                vec![
                    tui::ui::shortcut(theme, "tab", "section"),
                    tui::ui::shortcut(theme, "↑/↓", "navigate"),
                    tui::ui::shortcut(theme, "enter", "show"),
                    tui::ui::shortcut(theme, "o", "open"),
                    tui::ui::shortcut(theme, "q", "quit"),
                ],
            ),
        )]
        .iter()
        .cloned()
        .collect()
    }

    fn update_context(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let state = app.state(&Cid::List(ListCid::IssueBrowser))?;
        let progress = match state {
            State::Tup2((StateValue::Usize(step), StateValue::Usize(total))) => {
                Progress::Step(step.saturating_add(1), total)
            }
            _ => Progress::None,
        };
        let context = ui::browse_context(context, theme, progress);
        app.remount(Cid::List(ListCid::Context), context.to_boxed(), vec![])?;

        Ok(())
    }

    fn update_shortcuts(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        cid: ListCid,
    ) -> Result<()> {
        if let Some(shortcuts) = self.shortcuts.get(&cid) {
            app.remount(
                Cid::List(ListCid::Shortcuts),
                shortcuts.clone().to_boxed(),
                vec![],
            )?;
        }
        Ok(())
    }
}

impl ViewPage<Cid, Message> for ListPage {
    fn mount(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let navigation = ui::list_navigation(theme);
        let header = tui::ui::app_header(context, theme, Some(navigation)).to_boxed();
        let issue_browser = ui::issues(context, theme, None).to_boxed();

        app.remount(Cid::List(ListCid::Header), header, vec![])?;
        app.remount(Cid::List(ListCid::IssueBrowser), issue_browser, vec![])?;

        app.active(&Cid::List(self.active_component.clone()))?;
        self.update_shortcuts(app, self.active_component.clone())?;
        self.update_context(app, context, theme)?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::List(ListCid::Header))?;
        app.umount(&Cid::List(ListCid::IssueBrowser))?;
        app.umount(&Cid::List(ListCid::Context))?;
        app.umount(&Cid::List(ListCid::Shortcuts))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        message: Message,
    ) -> Result<Option<Message>> {
        if let Message::Issue(IssueMessage::Reload(id)) = message {
            let selected = match id {
                Some(id) => cob::issue::find(context.repository(), &id)?.map(|issue| (id, issue)),
                _ => None,
            };

            let issue_browser = ui::issues(context, theme, selected).to_boxed();
            app.remount(Cid::List(ListCid::IssueBrowser), issue_browser, vec![])?;

            self.activate(app, ListCid::IssueBrowser)?;
        }

        self.update_context(app, context, theme)?;

        Ok(None)
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let shortcuts_h = 1u16;
        let layout = layout::default_page(area, shortcuts_h);

        app.view(&Cid::List(ListCid::Header), frame, layout.navigation);
        app.view(
            &Cid::List(self.active_component.clone()),
            frame,
            layout.component,
        );

        app.view(&Cid::List(ListCid::Shortcuts), frame, layout.shortcuts);
    }

    fn subscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.subscribe(
            &Cid::List(ListCid::Header),
            Sub::new(subscription::navigation_clause(), SubClause::Always),
        )?;

        Ok(())
    }

    fn unsubscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.unsubscribe(
            &Cid::List(ListCid::Header),
            subscription::navigation_clause(),
        )?;

        Ok(())
    }
}

///
/// Issue detail page
///
pub struct IssuePage {
    issue: Option<(IssueId, Issue)>,
    active_component: IssueCid,
    shortcuts: HashMap<IssueCid, Widget<Shortcuts>>,
}

impl IssuePage {
    pub fn new(_context: &Context, theme: &Theme, issue: Option<(IssueId, Issue)>) -> Self {
        let shortcuts = Self::build_shortcuts(theme);
        let active_component = IssueCid::List;

        Self {
            issue,
            active_component,
            shortcuts,
        }
    }

    fn activate(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        cid: IssueCid,
    ) -> Result<()> {
        self.active_component = cid;
        let cid = Cid::Issue(self.active_component.clone());
        app.active(&cid)?;
        app.attr(&cid, Attribute::Focus, AttrValue::Flag(true))?;

        Ok(())
    }

    fn build_shortcuts(theme: &Theme) -> HashMap<IssueCid, Widget<Shortcuts>> {
        [
            (
                IssueCid::List,
                tui::ui::shortcuts(
                    theme,
                    vec![
                        tui::ui::shortcut(theme, "esc", "back"),
                        tui::ui::shortcut(theme, "↑/↓", "navigate"),
                        tui::ui::shortcut(theme, "enter", "show"),
                        tui::ui::shortcut(theme, "o", "open"),
                        tui::ui::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
            (
                IssueCid::Details,
                tui::ui::shortcuts(
                    theme,
                    vec![
                        tui::ui::shortcut(theme, "esc", "back"),
                        tui::ui::shortcut(theme, "↑/↓", "scroll"),
                        tui::ui::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
            (
                IssueCid::Form,
                tui::ui::shortcuts(
                    theme,
                    vec![
                        tui::ui::shortcut(theme, "esc", "back"),
                        tui::ui::shortcut(theme, "shift + tab / tab", "navigate"),
                        tui::ui::shortcut(theme, "ctrl + s", "submit"),
                    ],
                ),
            ),
        ]
        .iter()
        .cloned()
        .collect()
    }

    fn update_context(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        cid: IssueCid,
    ) -> Result<()> {
        let context = match cid {
            IssueCid::List => {
                let state = app.state(&Cid::Issue(IssueCid::List))?;
                let progress = match state {
                    State::Tup2((StateValue::Usize(step), StateValue::Usize(total))) => {
                        Progress::Step(step.saturating_add(1), total)
                    }
                    _ => Progress::None,
                };
                let context = ui::browse_context(context, theme, progress);
                Some(context)
            }
            IssueCid::Details => {
                let state = app.state(&Cid::Issue(IssueCid::Details))?;
                let progress = match state {
                    State::One(StateValue::Usize(scroll)) => Progress::Percentage(scroll),
                    _ => Progress::None,
                };
                let context = ui::description_context(context, theme, progress);
                Some(context)
            }
            IssueCid::Form => {
                let context = ui::form_context(context, theme, Progress::None);
                Some(context)
            }
            _ => None,
        };

        if let Some(context) = context {
            app.remount(Cid::Issue(IssueCid::Context), context.to_boxed(), vec![])?;
        }

        Ok(())
    }

    fn update_shortcuts(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        cid: IssueCid,
    ) -> Result<()> {
        if let Some(shortcuts) = self.shortcuts.get(&cid) {
            app.remount(
                Cid::Issue(IssueCid::Shortcuts),
                shortcuts.clone().to_boxed(),
                vec![],
            )?;
        }
        Ok(())
    }
}

impl ViewPage<Cid, Message> for IssuePage {
    fn mount(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let navigation = ui::list_navigation(theme);
        let header = tui::ui::app_header(context, theme, Some(navigation)).to_boxed();
        let list = ui::list(context, theme, self.issue.clone()).to_boxed();

        app.remount(Cid::Issue(IssueCid::Header), header, vec![])?;
        app.remount(Cid::Issue(IssueCid::List), list, vec![])?;

        if let Some((id, issue)) = &self.issue {
            let comments = issue.comments().collect::<Vec<_>>();
            let details = ui::details(
                context,
                theme,
                (*id, issue.clone()),
                comments.first().copied(),
            )
            .to_boxed();
            app.remount(Cid::Issue(IssueCid::Details), details, vec![])?;
        }

        app.active(&Cid::Issue(self.active_component.clone()))?;

        self.update_shortcuts(app, self.active_component.clone())?;
        self.update_context(app, context, theme, self.active_component.clone())?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::Issue(IssueCid::Header))?;
        app.umount(&Cid::Issue(IssueCid::List))?;
        app.umount(&Cid::Issue(IssueCid::Context))?;
        app.umount(&Cid::Issue(IssueCid::Shortcuts))?;

        if app.mounted(&Cid::Issue(IssueCid::Details)) {
            app.umount(&Cid::Issue(IssueCid::Details))?;
        }

        if app.mounted(&Cid::Issue(IssueCid::Form)) {
            app.umount(&Cid::Issue(IssueCid::Form))?;
        }

        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        message: Message,
    ) -> Result<Option<Message>> {
        match message {
            Message::Issue(IssueMessage::Created(id)) => {
                let repo = context.repository();

                if let Some(issue) = cob::issue::find(repo, &id)? {
                    self.issue = Some((id, issue.clone()));
                    let list = ui::list(context, theme, self.issue.clone()).to_boxed();
                    let comments = issue.comments().collect::<Vec<_>>();

                    let details = ui::details(
                        context,
                        theme,
                        (id, issue.clone()),
                        comments.first().copied(),
                    )
                    .to_boxed();

                    app.remount(Cid::Issue(IssueCid::List), list, vec![])?;
                    app.remount(Cid::Issue(IssueCid::Details), details, vec![])?;
                }
            }
            Message::Issue(IssueMessage::Changed(id)) => {
                let repo = context.repository();
                if let Some(issue) = cob::issue::find(repo, &id)? {
                    self.issue = Some((id, issue.clone()));
                    let comments = issue.comments().collect::<Vec<_>>();
                    let details = ui::details(
                        context,
                        theme,
                        (id, issue.clone()),
                        comments.first().copied(),
                    )
                    .to_boxed();
                    app.remount(Cid::Issue(IssueCid::Details), details, vec![])?;
                }
            }
            Message::Issue(IssueMessage::Focus(cid)) => {
                self.activate(app, cid)?;
                self.update_shortcuts(app, self.active_component.clone())?;
            }
            Message::Issue(IssueMessage::OpenForm) => {
                let new_form = ui::new_form(context, theme).to_boxed();
                let list = ui::list(context, theme, None).to_boxed();

                app.remount(Cid::Issue(IssueCid::List), list, vec![])?;
                app.remount(Cid::Issue(IssueCid::Form), new_form, vec![])?;
                app.active(&Cid::Issue(IssueCid::Form))?;

                app.unsubscribe(&Cid::GlobalListener, subscription::global_clause())?;

                return Ok(Some(Message::Issue(IssueMessage::Focus(IssueCid::Form))));
            }
            Message::Issue(IssueMessage::HideForm) => {
                app.umount(&Cid::Issue(IssueCid::Form))?;

                let list = ui::list(context, theme, self.issue.clone()).to_boxed();
                app.remount(Cid::Issue(IssueCid::List), list, vec![])?;

                app.subscribe(
                    &Cid::GlobalListener,
                    Sub::new(subscription::global_clause(), SubClause::Always),
                )?;

                if self.issue.is_none() {
                    return Ok(Some(Message::Issue(IssueMessage::Leave(None))));
                }
                return Ok(Some(Message::Issue(IssueMessage::Focus(IssueCid::List))));
            }
            Message::FormSubmitted(id) => {
                if id == ui::FORM_ID_EDIT {
                    let state = app.state(&Cid::Issue(IssueCid::Form))?;
                    if let State::Linked(mut states) = state {
                        let mut missing_values = vec![];

                        let title = match states.front() {
                            Some(State::One(StateValue::String(title))) if !title.is_empty() => {
                                Some(title.clone())
                            }
                            _ => None,
                        };
                        states.pop_front();

                        let tags = match states.front() {
                            Some(State::One(StateValue::String(tags))) => Some(tags.clone()),
                            _ => Some(String::from("[]")),
                        };
                        states.pop_front();

                        let assignees = match states.front() {
                            Some(State::One(StateValue::String(assignees))) => {
                                Some(assignees.clone())
                            }
                            _ => Some(String::from("[]")),
                        };
                        states.pop_front();

                        let description = match states.front() {
                            Some(State::One(StateValue::String(description)))
                                if !description.is_empty() =>
                            {
                                Some(description.clone())
                            }
                            _ => None,
                        };
                        states.pop_front();

                        if title.is_none() {
                            missing_values.push("title");
                        }
                        if description.is_none() {
                            missing_values.push("description");
                        }

                        // show error popup if missing.
                        if !missing_values.is_empty() {
                            let error = format!("Missing fields: {:?}", missing_values);
                            return Ok(Some(Message::Popup(PopupMessage::Error(error))));
                        } else {
                            return Ok(Some(Message::Issue(IssueMessage::Cob(
                                IssueCobMessage::Create {
                                    title: title.unwrap(),
                                    tags: tags.unwrap(),
                                    assignees: assignees.unwrap(),
                                    description: description.unwrap(),
                                },
                            ))));
                        }
                    }
                }
            }
            _ => {}
        }

        self.update_context(app, context, theme, self.active_component.clone())?;

        Ok(None)
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let shortcuts_h = 1u16;
        let layout = layout::issue_page(area, shortcuts_h);

        app.view(&Cid::Issue(IssueCid::Header), frame, layout.header);
        app.view(&Cid::Issue(IssueCid::List), frame, layout.left);

        if app.mounted(&Cid::Issue(IssueCid::Form)) {
            app.view(&Cid::Issue(IssueCid::Form), frame, layout.right);
        } else if app.mounted(&Cid::Issue(IssueCid::Details)) {
            app.view(&Cid::Issue(IssueCid::Details), frame, layout.right);
        }

        app.view(&Cid::Issue(IssueCid::Context), frame, layout.context);
        app.view(&Cid::Issue(IssueCid::Shortcuts), frame, layout.shortcuts);
    }

    fn subscribe(&self, _app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        Ok(())
    }

    fn unsubscribe(&self, _app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        Ok(())
    }
}

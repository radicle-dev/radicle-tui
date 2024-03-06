use std::collections::HashMap;

use anyhow::Result;

use tuirealm::{AttrValue, Attribute, Frame, NoUserEvent};

use radicle_tui as tui;

use tui::common::cob::inbox::{Filter, SortBy};
use tui::common::context::Context;
use tui::realm::ui::layout;
use tui::realm::ui::state::ItemState;
use tui::realm::ui::theme::Theme;
use tui::realm::ui::widget::context::{Progress, Shortcuts};
use tui::realm::ui::widget::Widget;
use tui::realm::ViewPage;

use crate::tui_inbox::common::SelectionMode;

use super::{ui, Application, Cid, ListCid, Message, Mode};

///
/// Home
///
pub struct ListView {
    active_component: ListCid,
    mode: Mode,
    filter: Filter,
    sort_by: SortBy,
    shortcuts: HashMap<ListCid, Widget<Shortcuts>>,
}

impl ListView {
    pub fn new(mode: Mode, filter: Filter, sort_by: SortBy) -> Self {
        Self {
            active_component: ListCid::NotificationBrowser,
            mode,
            filter,
            sort_by,
            shortcuts: HashMap::default(),
        }
    }

    fn update_context(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let state = app.state(&Cid::List(ListCid::NotificationBrowser))?;
        let progress = match ItemState::try_from(state) {
            Ok(state) => Progress::Step(
                state
                    .selected()
                    .map(|s| s.saturating_add(1))
                    .unwrap_or_default(),
                state.len(),
            ),
            Err(_) => Progress::None,
        };

        let context = ui::browse_context(context, theme, self.filter.clone(), progress);

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

impl ViewPage<Cid, Message> for ListView {
    fn mount(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let browser = ui::operation_select(theme, context, self.filter.clone(), self.sort_by, None)
            .to_boxed();
        self.shortcuts = browser.as_ref().shortcuts();

        match self.mode.selection() {
            SelectionMode::Id => {
                let notif_browser =
                    ui::id_select(theme, context, self.filter.clone(), self.sort_by, None)
                        .to_boxed();
                self.shortcuts = notif_browser.as_ref().shortcuts();

                app.remount(Cid::List(ListCid::NotificationBrowser), browser, vec![])?;
            }
            SelectionMode::Operation => {
                let notif_browser =
                    ui::operation_select(theme, context, self.filter.clone(), self.sort_by, None)
                        .to_boxed();
                self.shortcuts = notif_browser.as_ref().shortcuts();

                app.remount(Cid::List(ListCid::NotificationBrowser), browser, vec![])?;
            }
        };

        app.active(&Cid::List(self.active_component.clone()))?;
        self.update_shortcuts(app, self.active_component.clone())?;
        self.update_context(app, context, theme)?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::List(ListCid::NotificationBrowser))?;
        app.umount(&Cid::List(ListCid::Context))?;
        app.umount(&Cid::List(ListCid::Shortcuts))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        _message: Message,
    ) -> Result<Option<Message>> {
        self.update_context(app, context, theme)?;

        Ok(None)
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let context_h = app
            .query(&Cid::List(ListCid::Context), Attribute::Height)
            .unwrap_or_default()
            .unwrap_or(AttrValue::Size(0))
            .unwrap_size();
        let shortcuts_h = 1u16;

        let layout = layout::default_page(area, context_h, shortcuts_h);

        app.view(
            &Cid::List(self.active_component.clone()),
            frame,
            layout.component,
        );

        app.view(&Cid::List(ListCid::Context), frame, layout.context);
        app.view(&Cid::List(ListCid::Shortcuts), frame, layout.shortcuts);
    }

    fn subscribe(&self, _app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        Ok(())
    }

    fn unsubscribe(&self, _app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        Ok(())
    }
}

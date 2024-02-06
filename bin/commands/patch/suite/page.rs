use std::collections::HashMap;

use anyhow::Result;

use radicle::cob::patch::{Patch, PatchId};

use tui::ui::state::ItemState;
use tuirealm::{AttrValue, Attribute, Frame, NoUserEvent, Sub, SubClause};

use radicle_tui as tui;

use tui::cob::patch::Filter;
use tui::context::Context;
use tui::ui::theme::Theme;
use tui::ui::widget::context::{Progress, Shortcuts};
use tui::ui::widget::Widget;
use tui::ui::{layout, subscription};
use tui::ViewPage;

use super::{ui, Application, Cid, ListCid, Message, PatchCid};

///
/// Home
///
pub struct ListView {
    active_component: ListCid,
    shortcuts: HashMap<ListCid, Widget<Shortcuts>>,
    filter: Filter,
}

impl ListView {
    pub fn new(theme: Theme, filter: Filter) -> Self {
        let shortcuts = Self::build_shortcuts(&theme);
        Self {
            active_component: ListCid::PatchBrowser,
            shortcuts,
            filter,
        }
    }

    fn build_shortcuts(theme: &Theme) -> HashMap<ListCid, Widget<Shortcuts>> {
        [(
            ListCid::PatchBrowser,
            tui::ui::shortcuts(
                theme,
                vec![
                    tui::ui::shortcut(theme, "tab", "section"),
                    tui::ui::shortcut(theme, "↑/↓", "navigate"),
                    tui::ui::shortcut(theme, "enter", "show"),
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
        let state = app.state(&Cid::List(ListCid::PatchBrowser))?;
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

impl ViewPage<Cid, Message> for ListView {
    fn mount(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let navigation = ui::list_navigation(theme);
        let header = tui::ui::app_header(context, theme, Some(navigation)).to_boxed();
        let patch_browser = ui::patches(theme, context, self.filter.clone(), None).to_boxed();

        app.remount(Cid::List(ListCid::Header), header, vec![])?;
        app.remount(Cid::List(ListCid::PatchBrowser), patch_browser, vec![])?;

        app.active(&Cid::List(self.active_component.clone()))?;
        self.update_shortcuts(app, self.active_component.clone())?;
        self.update_context(app, context, theme)?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::List(ListCid::Header))?;
        app.umount(&Cid::List(ListCid::PatchBrowser))?;
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

        let layout = layout::full_page(area, context_h, shortcuts_h);

        app.view(&Cid::List(ListCid::Header), frame, layout.navigation);
        app.view(
            &Cid::List(self.active_component.clone()),
            frame,
            layout.component,
        );

        app.view(&Cid::List(ListCid::Context), frame, layout.context);
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
/// Patch detail page
///
pub struct PatchView {
    active_component: PatchCid,
    patch: (PatchId, Patch),
    shortcuts: HashMap<PatchCid, Widget<Shortcuts>>,
}

impl PatchView {
    pub fn new(theme: Theme, patch: (PatchId, Patch)) -> Self {
        let shortcuts = Self::build_shortcuts(&theme);
        PatchView {
            active_component: PatchCid::Activity,
            patch,
            shortcuts,
        }
    }

    fn build_shortcuts(theme: &Theme) -> HashMap<PatchCid, Widget<Shortcuts>> {
        [
            (
                PatchCid::Activity,
                tui::ui::shortcuts(
                    theme,
                    vec![
                        tui::ui::shortcut(theme, "esc", "back"),
                        tui::ui::shortcut(theme, "tab", "section"),
                        tui::ui::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
            (
                PatchCid::Files,
                tui::ui::shortcuts(
                    theme,
                    vec![
                        tui::ui::shortcut(theme, "esc", "back"),
                        tui::ui::shortcut(theme, "tab", "section"),
                        tui::ui::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
        ]
        .iter()
        .cloned()
        .collect()
    }

    fn update_shortcuts(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        cid: PatchCid,
    ) -> Result<()> {
        if let Some(shortcuts) = self.shortcuts.get(&cid) {
            app.remount(
                Cid::Patch(PatchCid::Shortcuts),
                shortcuts.clone().to_boxed(),
                vec![],
            )?;
        }
        Ok(())
    }
}

impl ViewPage<Cid, Message> for PatchView {
    fn mount(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let navigation = ui::navigation(theme);
        let header = tui::ui::app_header(context, theme, Some(navigation)).to_boxed();
        let activity = ui::activity(theme).to_boxed();
        let files = ui::files(theme).to_boxed();
        let context = ui::context(context, theme, self.patch.clone()).to_boxed();

        app.remount(Cid::Patch(PatchCid::Header), header, vec![])?;
        app.remount(Cid::Patch(PatchCid::Activity), activity, vec![])?;
        app.remount(Cid::Patch(PatchCid::Files), files, vec![])?;
        app.remount(Cid::Patch(PatchCid::Context), context, vec![])?;

        let active_component = Cid::Patch(self.active_component.clone());
        app.active(&active_component)?;
        self.update_shortcuts(app, self.active_component.clone())?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::Patch(PatchCid::Header))?;
        app.umount(&Cid::Patch(PatchCid::Activity))?;
        app.umount(&Cid::Patch(PatchCid::Files))?;
        app.umount(&Cid::Patch(PatchCid::Context))?;
        app.umount(&Cid::Patch(PatchCid::Shortcuts))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        _context: &Context,
        _theme: &Theme,
        message: Message,
    ) -> Result<Option<Message>> {
        if let Message::NavigationChanged(index) = message {
            self.active_component = PatchCid::from(index as usize);

            let active_component = Cid::Patch(self.active_component.clone());
            app.active(&active_component)?;
            self.update_shortcuts(app, self.active_component.clone())?;
        }

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

        let layout = layout::full_page(area, context_h, shortcuts_h);

        app.view(&Cid::Patch(PatchCid::Header), frame, layout.navigation);
        app.view(
            &Cid::Patch(self.active_component.clone()),
            frame,
            layout.component,
        );
        app.view(&Cid::Patch(PatchCid::Context), frame, layout.context);
        app.view(&Cid::Patch(PatchCid::Shortcuts), frame, layout.shortcuts);
    }

    fn subscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.subscribe(
            &Cid::Patch(PatchCid::Header),
            Sub::new(subscription::navigation_clause(), SubClause::Always),
        )?;

        Ok(())
    }

    fn unsubscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.unsubscribe(
            &Cid::Patch(PatchCid::Header),
            subscription::navigation_clause(),
        )?;

        Ok(())
    }
}

impl From<usize> for PatchCid {
    fn from(index: usize) -> Self {
        match index {
            0 => PatchCid::Activity,
            1 => PatchCid::Files,
            _ => PatchCid::Activity,
        }
    }
}

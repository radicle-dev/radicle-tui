use std::collections::HashMap;

use anyhow::Result;

use tuirealm::{AttrValue, Attribute, Frame, NoUserEvent, State, StateValue, Sub, SubClause};

use radicle_tui as tui;

use tui::cob::patch::Filter;
use tui::context::Context;
use tui::ui::theme::Theme;
use tui::ui::widget::context::{Progress, Shortcuts};
use tui::ui::widget::Widget;
use tui::ui::{layout, subscription};
use tui::ViewPage;

use super::super::common;
use super::{ui, Application, Cid, ListCid, Message, Mode};

///
/// Home
///
pub struct ListView {
    active_component: ListCid,
    subject: Mode,
    filter: Filter,
    shortcuts: HashMap<ListCid, Widget<Shortcuts>>,
}

impl ListView {
    pub fn new(subject: Mode, filter: Filter) -> Self {
        Self {
            active_component: ListCid::PatchBrowser,
            subject,
            filter,
            shortcuts: HashMap::default(),
        }
    }

    fn update_context(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let state = app.state(&Cid::List(ListCid::PatchBrowser))?;
        let progress = match state {
            State::Tup2((StateValue::Usize(step), StateValue::Usize(total))) => {
                Progress::Step(step.saturating_add(1), total)
            }
            _ => Progress::None,
        };
        let context = common::ui::browse_context(context, theme, self.filter.clone(), progress);

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

        app.remount(Cid::List(ListCid::Header), header, vec![])?;

        match self.subject {
            Mode::Id => {
                let patch_browser =
                    ui::id_select(theme, context, self.filter.clone(), None).to_boxed();
                self.shortcuts = patch_browser.as_ref().shortcuts();

                app.remount(Cid::List(ListCid::PatchBrowser), patch_browser, vec![])?;
            }
            Mode::Operation => {
                let patch_browser =
                    ui::operation_select(theme, context, self.filter.clone(), None).to_boxed();
                self.shortcuts = patch_browser.as_ref().shortcuts();

                app.remount(Cid::List(ListCid::PatchBrowser), patch_browser, vec![])?;
            }
        };

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

        let layout = layout::default_page(area, context_h, shortcuts_h);

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

use super::*;
use crate::common::page::{
    INIT_H, INIT_W, INNER_W, PAGE_APP, PAGE_COUNT, PAGE_DATA, PAGE_FEEDBACK, PAGE_GENERAL,
    PAGE_INPUT, PAGE_OTHER, PAGE_TITLES, SIDEBAR_W,
};
use uix::prelude::{
    dynamic_label, Button, DesignTokens, Form, FormItem, Input, Label, Rect, SelectableList, State,
    SystemEvent, Table,
};
use uix::ui::test_harness::{ViewAdapter, WidgetCore};
use uix::ui::traits::WidgetLayout;
use uix::ui::widgets::ScrollView;

mod layout_and_interaction;
mod navigation_structure;
mod reconcile_and_state;
mod window_title_bar;

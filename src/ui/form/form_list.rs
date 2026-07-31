//! 动态表单行的应用侧模型。

use std::fmt;

use crate::ui::view::{View, ViewNode};
use crate::ui::State;

use crate::ui::form::{Form, FormBuilder, FormField, FormModel, IntoFormValue, Values};

/// 动态表单行的稳定业务身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormListItemId(u64);

impl FormListItemId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// 动态表单结构变更失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormListError {
    ItemIdExhausted,
}

impl fmt::Display for FormListError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ItemIdExhausted => formatter.write_str("form list item id exhausted"),
        }
    }
}

impl std::error::Error for FormListError {}

/// 单行中的首个字段错误，并保留行身份与当前顺序。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormListFieldError {
    item_id: FormListItemId,
    item_index: usize,
    field: String,
    message: String,
}

impl FormListFieldError {
    pub fn item_id(&self) -> FormListItemId {
        self.item_id
    }

    pub fn item_index(&self) -> usize {
        self.item_index
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for FormListFieldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}[{}].{}: {}",
            self.item_id.get(),
            self.item_index,
            self.field,
            self.message
        )
    }
}

impl std::error::Error for FormListFieldError {}

/// 一次成功列表校验得到的逐行 typed values。
#[derive(Debug, Clone, Default)]
pub struct FormListValues {
    items: Vec<(FormListItemId, Values)>,
}

impl FormListValues {
    pub fn get(&self, item_id: FormListItemId) -> Option<&Values> {
        self.items
            .iter()
            .find_map(|(id, values)| (*id == item_id).then_some(values))
    }

    pub fn item(&self, index: usize) -> Option<(FormListItemId, &Values)> {
        self.items.get(index).map(|(id, values)| (*id, values))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// `Form::list` 配置闭包的字段入口。
pub struct FormListFields {
    layout: Form,
}

impl FormListFields {
    pub fn field(self, name: impl Into<String>, label: impl Into<String>) -> FormBuilder {
        FormBuilder::new(self.layout, name, label)
    }
}

/// 动态表单声明构建器。
pub struct FormListBuilder {
    name: String,
    layout: Form,
    schema: Vec<FormField>,
}

impl FormListBuilder {
    fn new(name: impl Into<String>, fields: FormBuilder) -> Self {
        let (layout, schema) = fields.into_parts();
        Self {
            name: name.into(),
            layout,
            schema,
        }
    }

    pub fn build(self) -> FormListModel {
        FormListModel {
            name: self.name,
            layout: self.layout,
            schema: self.schema,
            items: Vec::new(),
            item_ids: State::new(Vec::new()),
            next_id: 1,
        }
    }
}

struct FormListItem {
    id: FormListItemId,
    model: FormModel,
}

/// 应用侧动态表单模型；每行复用声明规则但独立保存值和触发错误。
pub struct FormListModel {
    name: String,
    layout: Form,
    schema: Vec<FormField>,
    items: Vec<FormListItem>,
    item_ids: State<Vec<FormListItemId>>,
    next_id: u64,
}

impl FormListModel {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn layout(&self) -> &Form {
        &self.layout
    }

    /// 新增一行；结构 State 会通知已经读取该列表的 View。
    pub fn add_item(&mut self) -> Result<FormListItemId, FormListError> {
        let id = FormListItemId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(FormListError::ItemIdExhausted)?;
        self.items.push(FormListItem {
            id,
            model: FormModel::from_fields(self.layout.clone(), self.schema.clone()),
        });
        self.publish_item_ids();
        Ok(id)
    }

    /// 按当前顺序删除一行，返回被删除行的稳定身份。
    pub fn remove_item(&mut self, index: usize) -> Option<FormListItemId> {
        if index >= self.items.len() {
            return None;
        }
        let removed = self.items.remove(index).id;
        self.publish_item_ids();
        Some(removed)
    }

    pub fn clear(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.items.clear();
        self.publish_item_ids();
    }

    /// 返回当前顺序；在 View 构建期读取时自动登记结构 reconcile。
    pub fn item_ids(&self) -> Vec<FormListItemId> {
        self.item_ids.get()
    }

    /// 把当前行映射为带稳定 key 的 View；结构增删会自动请求所属根 View reconcile。
    pub fn render_rows<V>(
        &self,
        mut render: impl FnMut(FormListItemId, usize) -> V,
    ) -> Vec<ViewNode>
    where
        V: View,
    {
        self.item_ids()
            .into_iter()
            .enumerate()
            .map(|(index, item_id)| render(item_id, index).build().key(self.item_key(item_id)))
            .collect()
    }

    pub fn item_key(&self, item_id: FormListItemId) -> String {
        format!("form-list:{}:{}", self.name, item_id.get())
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn set_value<V: IntoFormValue>(
        &mut self,
        item_id: FormListItemId,
        field: &str,
        value: V,
    ) -> bool {
        self.items
            .iter_mut()
            .find(|item| item.id == item_id)
            .is_some_and(|item| item.model.set_value(field, value))
    }

    pub fn blur(&mut self, item_id: FormListItemId, field: &str) -> bool {
        self.items
            .iter_mut()
            .find(|item| item.id == item_id)
            .is_some_and(|item| item.model.blur(field))
    }

    /// 逐行校验；错误按当前行顺序、再按字段声明顺序返回。
    pub fn validate(&self) -> Result<FormListValues, Vec<FormListFieldError>> {
        let mut values = Vec::with_capacity(self.items.len());
        let mut errors = Vec::new();
        for (item_index, item) in self.items.iter().enumerate() {
            match item.model.validate() {
                Ok(item_values) => values.push((item.id, item_values)),
                Err(item_errors) => errors.extend(item_errors.into_iter().map(|error| {
                    let (field, message) = error.into_parts();
                    FormListFieldError {
                        item_id: item.id,
                        item_index,
                        field,
                        message,
                    }
                })),
            }
        }
        if errors.is_empty() {
            Ok(FormListValues { items: values })
        } else {
            Err(errors)
        }
    }

    fn publish_item_ids(&self) {
        self.item_ids
            .set(self.items.iter().map(|item| item.id).collect());
    }
}

impl Form {
    /// 声明一组可动态增删、逐行校验的同构字段。
    pub fn list(
        self,
        name: impl Into<String>,
        configure: impl FnOnce(FormListFields) -> FormBuilder,
    ) -> FormListBuilder {
        let fields = configure(FormListFields { layout: self });
        FormListBuilder::new(name, fields)
    }
}

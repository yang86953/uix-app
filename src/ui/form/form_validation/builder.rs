//! 表单构建器。

use super::*;

/// 按声明顺序构造带类型值和验证规则的表单模型。
pub struct FormBuilder {
    layout: Form,
    fields: Vec<FormField>,
    current: FormField,
    initial_values: BTreeMap<String, StoredValue>,
}

impl FormBuilder {
    pub(crate) fn new(layout: Form, name: impl Into<String>, label: impl Into<String>) -> Self {
        Self::with_initial_values(layout, BTreeMap::new(), name, label)
    }

    fn with_initial_values(
        layout: Form,
        initial_values: BTreeMap<String, StoredValue>,
        name: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        let name = name.into();
        let mut current = FormField::new(name.clone(), label);
        if let Some(value) = initial_values.get(&name) {
            current.value = value.clone();
        }
        Self {
            layout,
            fields: Vec::new(),
            current,
            initial_values,
        }
    }

    /// 完成当前字段并开始声明下一个字段。
    pub fn field(mut self, name: impl Into<String>, label: impl Into<String>) -> Self {
        let name = name.into();
        let mut next = FormField::new(name.clone(), label);
        if let Some(value) = self.initial_values.get(&name) {
            next.value = value.clone();
        }
        self.fields.push(std::mem::replace(&mut self.current, next));
        self
    }

    /// 设置当前字段的初始值；[`FormModel::reset`] 会恢复到该值。
    pub fn initial<V: IntoFormValue>(mut self, value: V) -> Self {
        self.current.value = StoredValue::new(value);
        self
    }

    /// [`FormBuilder::initial`] 的兼容别名。
    pub fn default<V: IntoFormValue>(self, value: V) -> Self {
        self.initial(value)
    }

    /// 为当前字段添加必填规则及失败消息。
    pub fn required(mut self, message: impl Into<String>) -> Self {
        self.current.rules.push(FieldRule::Required(message.into()));
        self
    }

    /// 为当前字段添加电子邮箱格式规则及失败消息。
    pub fn validate_email(mut self, message: impl Into<String>) -> Self {
        self.current.rules.push(FieldRule::Email(message.into()));
        self
    }

    /// 为当前字段添加包含端点的数值范围规则及失败消息。
    pub fn validate_range<N>(mut self, range: RangeInclusive<N>, message: impl Into<String>) -> Self
    where
        N: Copy + Into<f64>,
    {
        self.current.rules.push(FieldRule::Range {
            start: (*range.start()).into(),
            end: (*range.end()).into(),
            message: message.into(),
        });
        self
    }

    /// 为当前字段添加包含端点的字符长度范围规则及失败消息。
    pub fn validate_length(
        mut self,
        range: RangeInclusive<usize>,
        message: impl Into<String>,
    ) -> Self {
        self.current.rules.push(FieldRule::Length {
            start: *range.start(),
            end: *range.end(),
            message: message.into(),
        });
        self
    }

    // 关闭表单 pattern capability 时同步收缩公开 builder 方法。
    #[cfg(feature = "form-pattern")]
    // 启用后把正则表达式预编译为字段规则。
    /// 为当前字段添加正则表达式规则及失败消息。
    pub fn validate_pattern(
        mut self,
        pattern: impl AsRef<str>,
        message: impl Into<String>,
    ) -> Self {
        // 非法正则不得静默降级为无规则：builder 层无诊断句柄，经边界
        // 观察入口保留开发者可定位的告警。
        let matcher = Regex::new(pattern.as_ref())
            .inspect_err(|error| {
                crate::diagnostics::observe_boundary_error(
                    "form/pattern",
                    &crate::core::Error::new(
                        crate::core::Errc::FormatError,
                        error.to_string(),
                    ),
                )
            })
            .ok();
        self.current.rules.push(FieldRule::Pattern {
            matcher,
            message: message.into(),
        });
        self
    }

    /// 为当前字段添加接收字符串投影的自定义验证器。
    pub fn custom(mut self, validator: impl Fn(&str) -> Result<(), String> + 'static) -> Self {
        self.current
            .rules
            .push(FieldRule::Custom(Arc::new(validator)));
        self
    }

    /// 声明当前字段依赖另一个 typed 字段；依赖源变化后会级联重验当前字段。
    pub fn depends_on<T>(
        mut self,
        field: impl Into<String>,
        validator: impl Fn(&T) -> Result<(), String> + 'static,
    ) -> Self
    where
        T: Send + Sync + 'static,
    {
        let field = field.into();
        let dependency = field.clone();
        self.current.rules.push(FieldRule::Dependency {
            field,
            validator: Arc::new(move |_, values| {
                let value = values.get::<T>(&dependency).ok_or_else(|| {
                    format!(
                        "dependency field `{dependency}` is missing or has an incompatible type"
                    )
                })?;
                validator(value)
            }),
        });
        self
    }

    /// 声明需要当前字段文本与整表 typed values 的高级依赖规则。
    pub fn depends_on_with_values(
        mut self,
        field: impl Into<String>,
        validator: impl Fn(&str, &Values) -> Result<(), String> + 'static,
    ) -> Self {
        self.current.rules.push(FieldRule::Dependency {
            field: field.into(),
            validator: Arc::new(validator),
        });
        self
    }

    /// 设置当前字段的规则激活时机。
    pub fn validate_trigger(mut self, trigger: Trigger) -> Self {
        self.current.trigger = trigger;
        self
    }

    /// 完成声明；同名字段以最后一次声明为准。
    pub fn build(self) -> FormModel {
        let (layout, fields) = self.into_parts();
        FormModel::from_fields(layout, fields)
    }

    pub(crate) fn into_parts(mut self) -> (Form, Vec<FormField>) {
        self.fields.push(self.current);
        let mut unique = Vec::<FormField>::with_capacity(self.fields.len());
        for field in self.fields {
            unique.retain(|existing| existing.name != field.name);
            unique.push(field);
        }
        (self.layout, unique)
    }
}

struct FormModelInner {
    layout: Form,
    fields: Vec<FormField>,
    values: RefCell<Vec<StoredValue>>,
    active_errors: State<Vec<Option<FieldError>>>,
    validation_active: RefCell<Vec<bool>>,
    focus_handles: RefCell<BTreeMap<String, FocusHandle>>,
    reset_bindings: RefCell<BTreeMap<String, Rc<dyn Fn()>>>,
}

/// 应用侧表单模型；克隆后仍共享字段值、校验状态与字段焦点句柄。
///
/// 自定义校验闭包只保留在这个应用侧句柄中，不进入 widget widget。
#[derive(Clone)]
pub struct FormModel {
    inner: Rc<FormModelInner>,
}

impl FormModel {
    pub(crate) fn from_fields(layout: Form, fields: Vec<FormField>) -> Self {
        let values = fields.iter().map(|field| field.value.clone()).collect();
        let field_count = fields.len();
        Self {
            inner: Rc::new(FormModelInner {
                layout,
                fields,
                values: RefCell::new(values),
                active_errors: State::new(vec![None; field_count]),
                validation_active: RefCell::new(vec![false; field_count]),
                focus_handles: RefCell::new(BTreeMap::new()),
                reset_bindings: RefCell::new(BTreeMap::new()),
            }),
        }
    }

    /// 校验全部字段；每个字段返回首个错误，字段间按声明顺序收集。
    pub fn validate(&self) -> Result<Values, Vec<FieldError>> {
        let current_values = self.inner.values.borrow();
        let values = values_from_fields(&self.inner.fields, &current_values);
        let field_errors: Vec<Option<FieldError>> = self
            .inner
            .fields
            .iter()
            .zip(current_values.iter())
            .map(|(field, value)| validate_field(field, value, &values))
            .collect();
        let errors = field_errors.iter().flatten().cloned().collect::<Vec<_>>();
        drop(current_values);
        self.inner.validation_active.borrow_mut().fill(true);
        self.publish_errors(field_errors);
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(values)
    }

    /// 更新字段值；字段不存在时返回 `false`。
    pub fn set_value<V: IntoFormValue>(&self, field: &str, value: V) -> bool {
        let Some(index) = self.inner.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        let mut current_values = self.inner.values.borrow_mut();
        current_values[index] = StoredValue::new(value);
        let values = values_from_fields(&self.inner.fields, &current_values);
        let dependents = dependent_indices(&self.inner.fields, index);
        let should_validate = {
            let mut validation_active = self.inner.validation_active.borrow_mut();
            if self.inner.fields[index].trigger == Trigger::OnChange {
                validation_active[index] = true;
            }
            for dependent in &dependents {
                validation_active[*dependent] = true;
            }
            validation_active[index]
        };
        let error = should_validate
            .then(|| validate_field(&self.inner.fields[index], &current_values[index], &values))
            .flatten();
        let mut active_errors = self.inner.active_errors.get_untracked();
        active_errors[index] = error;
        for dependent in dependents {
            active_errors[dependent] = validate_field(
                &self.inner.fields[dependent],
                &current_values[dependent],
                &values,
            );
        }
        drop(current_values);
        self.publish_errors(active_errors);
        true
    }

    /// 通知字段失焦；仅 `OnBlur` 字段会在此时执行规则。
    pub fn blur(&self, field: &str) -> bool {
        let Some(index) = self.inner.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        if self.inner.fields[index].trigger == Trigger::OnBlur {
            self.inner.validation_active.borrow_mut()[index] = true;
            let current_values = self.inner.values.borrow();
            let values = values_from_fields(&self.inner.fields, &current_values);
            let error = validate_field(&self.inner.fields[index], &current_values[index], &values);
            drop(current_values);
            let mut active_errors = self.inner.active_errors.get_untracked();
            active_errors[index] = error;
            self.publish_errors(active_errors);
        }
        true
    }

    /// 返回当前已激活的字段错误。
    pub fn field_error(&self, field: &str) -> Option<FieldError> {
        let index = self
            .inner
            .fields
            .iter()
            .position(|item| item.name == field)?;
        self.inner.active_errors.get().get(index).cloned().flatten()
    }

    /// 按字段声明顺序返回当前已激活的错误。
    pub fn errors(&self) -> Vec<FieldError> {
        self.inner
            .active_errors
            .get()
            .iter()
            .flatten()
            .cloned()
            .collect()
    }

    /// 返回指定字段的显示标签；字段不存在时返回 `None`。
    pub fn field_label(&self, field: &str) -> Option<&str> {
        self.inner
            .fields
            .iter()
            .find(|item| item.name == field)
            .map(|item| item.label.as_str())
    }

    /// 返回表单模型使用的布局配置。
    pub fn layout(&self) -> &Form {
        &self.inner.layout
    }

    /// 消耗模型并返回其布局配置。
    pub fn into_layout(self) -> Form {
        self.inner.layout.clone()
    }

    /// 校验整表；失败时尽力聚焦并显露首个错误字段。
    pub fn submit(&self) -> Result<Values, Vec<FieldError>> {
        let result = self.validate();
        if result.is_err() {
            let _ = self.scroll_to_first_error();
        }
        result
    }

    /// 恢复全部字段的声明初始值，清除已激活错误并同步已绑定的外部 State。
    pub fn reset(&self) {
        *self.inner.values.borrow_mut() = self
            .inner
            .fields
            .iter()
            .map(|field| field.value.clone())
            .collect();
        self.inner.validation_active.borrow_mut().fill(false);
        self.publish_errors(vec![None; self.inner.fields.len()]);

        let bindings = self
            .inner
            .reset_bindings
            .borrow()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for reset in bindings {
            reset();
        }
    }

    /// 把焦点登记到首个错误字段；当前无错误时返回 `Ok(None)`。
    pub fn focus_first_error(&self) -> Result<Option<String>, FocusHandleError> {
        self.request_first_error_focus(false)
    }

    /// 聚焦首个错误字段，并请求所有支持显露的祖先 viewport 滚动到该字段。
    pub fn scroll_to_first_error(&self) -> Result<Option<String>, FocusHandleError> {
        self.request_first_error_focus(true)
    }

    fn request_first_error_focus(&self, reveal: bool) -> Result<Option<String>, FocusHandleError> {
        let Some(error) = self.errors().into_iter().next() else {
            return Ok(None);
        };
        let field = error.field().to_string();
        let handle = self
            .inner
            .focus_handles
            .borrow()
            .get(&field)
            .cloned()
            .ok_or(FocusHandleError::Unbound)?;
        if reveal {
            handle.focus_and_reveal()?;
        } else {
            handle.focus()?;
        }
        Ok(Some(field))
    }

    pub(crate) fn has_field(&self, field: &str) -> bool {
        self.inner.fields.iter().any(|item| item.name == field)
    }

    pub(crate) fn field_is_required(&self, field: &str) -> bool {
        self.inner
            .fields
            .iter()
            .find(|item| item.name == field)
            .is_some_and(|item| {
                item.rules
                    .iter()
                    .any(|rule| matches!(rule, FieldRule::Required(_)))
            })
    }

    pub(crate) fn sync_text_value(&self, field: &str, value: &str) -> bool {
        let Some(index) = self.inner.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        if self.inner.values.borrow()[index].is_text(value) {
            return true;
        }
        self.set_value(field, value.to_string())
    }

    pub(crate) fn sync_typed_value<T>(&self, field: &str, value: &T) -> bool
    where
        T: Clone + PartialEq + Send + Sync + IntoFormValue<Stored = T> + 'static,
    {
        let Some(index) = self.inner.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        if self.inner.values.borrow()[index].is_value(value) {
            return true;
        }
        self.set_value(field, value.clone())
    }

    pub(crate) fn focus_handle_for(&self, field: &str) -> Option<FocusHandle> {
        if !self.has_field(field) {
            return None;
        }
        let mut handles = self.inner.focus_handles.borrow_mut();
        Some(handles.entry(field.to_string()).or_default().clone())
    }

    pub(crate) fn shared_focus_handle_for(&self, fields: &[&str]) -> Option<FocusHandle> {
        if fields.is_empty() || fields.iter().any(|field| !self.has_field(field)) {
            return None;
        }
        let mut handles = self.inner.focus_handles.borrow_mut();
        let handle = fields
            .iter()
            .find_map(|field| handles.get(*field).cloned())
            .unwrap_or_default();
        for field in fields {
            handles.insert((*field).to_string(), handle.clone());
        }
        Some(handle)
    }

    pub(crate) fn register_reset_state<T>(&self, field: &str, state: &State<T>) -> bool
    where
        T: Clone + PartialEq + Send + Sync + 'static,
    {
        let Some(index) = self.inner.fields.iter().position(|item| item.name == field) else {
            return false;
        };
        let Some(initial) = self.inner.fields[index].value.cloned::<T>() else {
            return false;
        };
        let state = state.clone();
        self.inner.reset_bindings.borrow_mut().insert(
            field.to_string(),
            Rc::new(move || {
                if state.get() != initial {
                    state.set(initial.clone());
                }
            }),
        );
        true
    }

    fn publish_errors(&self, errors: Vec<Option<FieldError>>) {
        if self.inner.active_errors.get_untracked() != errors {
            self.inner.active_errors.set(errors);
        }
    }
}

impl Form {
    /// 开始声明应用侧字段校验模型。
    pub fn field(self, name: impl Into<String>, label: impl Into<String>) -> FormBuilder {
        FormBuilder::new(self, name, label)
    }

    /// 预置一组同类型字段初始值，再开始字段声明。
    pub fn initial_values<I, K, V>(self, values: I) -> FormInitialValues
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: IntoFormValue,
    {
        let values = values
            .into_iter()
            .map(|(field, value)| (field.into(), StoredValue::new(value)))
            .collect();
        FormInitialValues {
            layout: self,
            values,
        }
    }
}

/// `Form::initial_values` 返回的字段声明入口。
pub struct FormInitialValues {
    layout: Form,
    values: BTreeMap<String, StoredValue>,
}

impl FormInitialValues {
    /// 使用预置初始值开始声明首个字段。
    pub fn field(self, name: impl Into<String>, label: impl Into<String>) -> FormBuilder {
        FormBuilder::with_initial_values(self.layout, self.values, name, label)
    }
}

fn values_from_fields(fields: &[FormField], values: &[StoredValue]) -> Values {
    let entries = fields
        .iter()
        .zip(values)
        .map(|(field, value)| (field.name.clone(), Arc::clone(&value.typed)))
        .collect();
    Values { entries }
}

fn dependent_indices(fields: &[FormField], changed: usize) -> Vec<usize> {
    let mut visited = vec![false; fields.len()];
    visited[changed] = true;
    let mut frontier = vec![fields[changed].name.as_str()];
    let mut dependents = Vec::new();

    while let Some(source) = frontier.pop() {
        for (index, field) in fields.iter().enumerate() {
            if visited[index] || !field.depends_on(source) {
                continue;
            }
            visited[index] = true;
            dependents.push(index);
            frontier.push(field.name.as_str());
        }
    }
    dependents.sort_unstable();
    dependents
}

impl FormField {
    fn depends_on(&self, source: &str) -> bool {
        self.rules.iter().any(|rule| {
            matches!(
                rule,
                FieldRule::Dependency { field, .. } if field == source
            )
        })
    }
}

fn validate_field(field: &FormField, current: &StoredValue, values: &Values) -> Option<FieldError> {
    for rule in &field.rules {
        let failed_message = match rule {
            FieldRule::Required(message) if current.is_empty() => Some(message.clone()),
            FieldRule::Email(message) if !current.is_empty() && !is_valid_email(&current.text) => {
                Some(message.clone())
            }
            FieldRule::Range {
                start,
                end,
                message,
            } if !current.is_empty()
                && current
                    .text
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite() && *value >= *start && *value <= *end)
                    .is_none() =>
            {
                Some(message.clone())
            }
            FieldRule::Length {
                start,
                end,
                message,
            } if !current.is_empty()
                && !(*start..=*end).contains(&current.text.chars().count()) =>
            {
                Some(message.clone())
            }
            // 关闭表单 pattern capability 时不生成无效正则分支。
            #[cfg(feature = "form-pattern")]
            // 无效表达式稳定返回调用方提供的错误文案。
            FieldRule::Pattern {
                matcher: None,
                message,
            } => Some(message.clone()),
            // 关闭表单 pattern capability 时不生成 matcher 执行分支。
            #[cfg(feature = "form-pattern")]
            // 非空文本不匹配时返回调用方提供的错误文案。
            FieldRule::Pattern {
                matcher: Some(matcher),
                message,
            } if !current.is_empty() && !matcher.is_match(&current.text) => Some(message.clone()),
            FieldRule::Custom(validator) => validator(&current.text).err(),
            FieldRule::Dependency { validator, .. } => validator(&current.text, values).err(),
            _ => None,
        };
        if let Some(message) = failed_message {
            return Some(FieldError::new(&field.name, message));
        }
    }
    None
}

fn is_valid_email(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = value.split('@');
    let Some(local) = parts.next() else {
        return false;
    };
    let Some(domain) = parts.next() else {
        return false;
    };
    if parts.next().is_some()
        || local.is_empty()
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return false;
    }
    domain.split('.').filter(|part| !part.is_empty()).count() >= 2
        && domain.split('.').all(|part| !part.is_empty())
}

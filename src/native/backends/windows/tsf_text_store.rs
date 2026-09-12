//! Minimal `ITextStoreACP` + `ITfContextOwnerCompositionSink`（P6 TSF phonetic）。
//!
//! TIP 经 lock 写入 UTF-16 缓冲；composition sink 把 marked/committed 收成 `UiEvent`。

#![cfg(windows)]

use std::cell::{Ref as CellRef, RefCell, RefMut as CellRefMut};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_UNEXPECTED, HWND, POINT, RECT};
use windows::Win32::UI::TextServices::{
    ITextStoreACP, ITextStoreACP_Impl, ITextStoreACPSink, ITfCompositionView,
    ITfContextOwnerCompositionSink, ITfContextOwnerCompositionSink_Impl, TS_E_NOLOCK,
    TS_E_SYNCHRONOUS, TS_IAS_NOQUERY, TS_IAS_QUERYONLY, TS_RT_PLAIN, TS_RUNINFO, TS_S_ASYNC,
    TS_SELECTION_ACP, TS_SS_NOHIDDENTEXT, TS_SS_TRANSITORY, TS_STATUS, TS_TEXTCHANGE,
};
use windows::core::{
    BOOL, ComObject, Error as WinError, GUID, HRESULT, Interface, PCWSTR, PWSTR, Ref,
    Result as WinResult, implement,
};

pub(crate) use super::tsf_document::{TsfEventSink, TsfLockKind, TsfLockRequest, TsfStoreState};
use crate::core::{Errc, Error};
use crate::native::backends::windows::tsf_session::tsf_composition_events;
use crate::platform::windowing::event::UiEvent;

const VIEW_ID: u32 = 0;

macro_rules! ffi_guard {
    // 事件接收器参数采用 Rust 2024 表达式片段语义。
    ($event_sink:expr, $operation:literal, $body:block) => {{
        let event_sink = $event_sink;
        match catch_unwind(AssertUnwindSafe(|| {
            panic_if_requested($operation);
            $body
        })) {
            Ok(result) => result,
            Err(_) => {
                event_sink.enqueue_failure(Error::new(
                    Errc::PlatformError,
                    format!("TSF ABI callback panicked: {}", $operation),
                ));
                Err(WinError::from(E_FAIL))
            }
        }
    }};
}

#[implement(ITextStoreACP, ITfContextOwnerCompositionSink)]
pub(crate) struct TsfTextStore {
    state: TsfStoreHandle,
}

impl TsfTextStore {
    pub(crate) fn new(state: TsfStoreHandle) -> Self {
        Self { state }
    }

    pub(crate) fn create(event_sink: TsfEventSink) -> (ComObject<Self>, TsfStoreHandle) {
        let state = TsfStoreHandle::new(TsfStoreState::new(event_sink));
        let store = ComObject::new(Self::new(state.clone()));
        (store, state)
    }
}

#[derive(Clone)]
pub(crate) struct TsfStoreHandle {
    state: Rc<RefCell<TsfStoreState>>,
    event_sink: TsfEventSink,
}

impl TsfStoreHandle {
    fn new(state: TsfStoreState) -> Self {
        let event_sink = state.event_sink.clone();
        Self {
            state: Rc::new(RefCell::new(state)),
            event_sink,
        }
    }

    pub(crate) fn read(&self) -> WinResult<CellRef<'_, TsfStoreState>> {
        self.state.try_borrow().map_err(|_| {
            self.event_sink.enqueue_failure(Error::new(
                Errc::InvalidState,
                "TSF: text store read borrow conflict",
            ));
            WinError::from(E_UNEXPECTED)
        })
    }

    pub(crate) fn write(&self) -> WinResult<CellRefMut<'_, TsfStoreState>> {
        self.state.try_borrow_mut().map_err(|_| {
            self.event_sink.enqueue_failure(Error::new(
                Errc::InvalidState,
                "TSF: text store write borrow conflict",
            ));
            WinError::from(E_UNEXPECTED)
        })
    }
}

fn lock_err() -> WinError {
    WinError::from(TS_E_NOLOCK)
}

fn require_pointer<T>(pointer: *const T) -> WinResult<()> {
    if pointer.is_null() {
        Err(WinError::from(E_INVALIDARG))
    } else {
        Ok(())
    }
}

fn require_buffer<T>(pointer: *const T, count: u32) -> WinResult<()> {
    if count > 0 {
        require_pointer(pointer)
    } else {
        Ok(())
    }
}

fn require_read_lock(state: &TsfStoreState) -> WinResult<()> {
    if state.has_read_lock() {
        Ok(())
    } else {
        Err(lock_err())
    }
}

fn require_write_lock(state: &TsfStoreState) -> WinResult<()> {
    if state.has_write_lock() {
        Ok(())
    } else {
        Err(lock_err())
    }
}

fn notify_lock_granted(
    event_sink: &TsfEventSink,
    sink: Option<&ITextStoreACPSink>,
    kind: TsfLockKind,
) -> HRESULT {
    let Some(sink) = sink else {
        return HRESULT(0);
    };
    // SAFETY: sink 为存活并已成功 AdviseSink 的 TSF sink 接口引用，本调用在 TSF owner 线程同步执行。
    match unsafe { sink.OnLockGranted(kind.flags()) } {
        Ok(()) => HRESULT(0),
        Err(error) => {
            let code = error.code();
            event_sink
                .enqueue_windows_failure("TSF: ITextStoreACPSink::OnLockGranted failed", error);
            code
        }
    }
}

impl ITextStoreACP_Impl for TsfTextStore_Impl {
    fn AdviseSink(
        &self,
        riid: *const GUID,
        punk: Ref<'_, windows::core::IUnknown>,
        dwmask: u32,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::AdviseSink",
            {
                require_pointer(riid)?;
                // SAFETY: riid 已由 require_pointer 验证非空且指向调用方存活的 GUID。
                let iid = unsafe { *riid };
                if iid != ITextStoreACPSink::IID {
                    return Ok(());
                }
                let punk = punk.ok()?;
                let sink: ITextStoreACPSink = punk.cast()?;
                let mut state = self.state.write()?;
                state.acp_sink = Some(sink);
                state.sink_mask = dwmask;
                Ok(())
            }
        )
    }

    fn UnadviseSink(&self, punk: Ref<'_, windows::core::IUnknown>) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::UnadviseSink",
            {
                let Ok(punk) = punk.ok() else {
                    return Ok(());
                };
                let mut state = self.state.write()?;
                if let Some(existing) = state.acp_sink.as_ref() {
                    if Interface::as_raw(existing) == Interface::as_raw(punk) {
                        state.acp_sink = None;
                        state.sink_mask = 0;
                    }
                }
                Ok(())
            }
        )
    }

    fn RequestLock(&self, dwlockflags: u32) -> WinResult<HRESULT> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::RequestLock",
            {
                let (event_sink, sink, request) = {
                    let mut state = self.state.write()?;
                    let request = state.begin_lock(dwlockflags);
                    (state.event_sink.clone(), state.acp_sink.clone(), request)
                };

                let kind = match request {
                    TsfLockRequest::Grant(kind) => kind,
                    TsfLockRequest::PendingWrite => return Ok(TS_S_ASYNC),
                    TsfLockRequest::RejectSynchronous => return Ok(TS_E_SYNCHRONOUS),
                };

                let session_hr = notify_lock_granted(&event_sink, sink.as_ref(), kind);
                let pending = self.state.write()?.complete_lock();
                if let Some(pending_kind) = pending {
                    let _ = notify_lock_granted(&event_sink, sink.as_ref(), pending_kind);
                    let _ = self.state.write()?.complete_lock();
                }
                Ok(session_hr)
            }
        )
    }

    fn GetStatus(&self) -> WinResult<TS_STATUS> {
        ffi_guard!(self.state.event_sink.clone(), "ITextStoreACP::GetStatus", {
            Ok(TS_STATUS {
                dwDynamicFlags: 0,
                dwStaticFlags: TS_SS_TRANSITORY | TS_SS_NOHIDDENTEXT,
            })
        })
    }

    fn QueryInsert(
        &self,
        acpteststart: i32,
        acptestend: i32,
        _cch: u32,
        pacpresultstart: *mut i32,
        pacpresultend: *mut i32,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::QueryInsert",
            {
                require_pointer(pacpresultstart)?;
                require_pointer(pacpresultend)?;
                let state = self.state.read()?;
                if !state.contains_range(acpteststart, acptestend) {
                    return Err(WinError::from(E_INVALIDARG));
                }
                // SAFETY: 两个输出指针均已由 require_pointer 验证非空，指向调用方分配的可写缓冲。
                unsafe {
                    *pacpresultstart = acpteststart;
                    *pacpresultend = acptestend;
                }
                Ok(())
            }
        )
    }

    fn GetSelection(
        &self,
        ulindex: u32,
        ulcount: u32,
        pselection: *mut TS_SELECTION_ACP,
        pcfetched: *mut u32,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::GetSelection",
            {
                if pcfetched.is_null() || (ulcount > 0 && pselection.is_null()) {
                    return Err(WinError::from(E_INVALIDARG));
                }
                let state = self.state.read()?;
                require_read_lock(&state)?;
                let mut fetched = 0u32;
                if ulcount > 0 && ulindex == 0 {
                    // SAFETY: pselection 非空已由上方条件验证，指向调用方分配的可写 TS_SELECTION_ACP。
                    unsafe {
                        *pselection = state.selection();
                    }
                    fetched = 1;
                }
                // SAFETY: pcfetched 非空已由上方条件验证，指向可写 u32。
                unsafe {
                    *pcfetched = fetched;
                }
                Ok(())
            }
        )
    }

    fn SetSelection(&self, ulcount: u32, pselection: *const TS_SELECTION_ACP) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::SetSelection",
            {
                if ulcount > 0 && pselection.is_null() {
                    return Err(WinError::from(E_INVALIDARG));
                }
                let mut state = self.state.write()?;
                require_write_lock(&state)?;
                if ulcount > 0 {
                    // SAFETY: pselection 非空已由上方条件验证，指向调用方存活的只读 TS_SELECTION_ACP。
                    let sel = unsafe { *pselection };
                    state.text_range(sel.acpStart, sel.acpEnd)?;
                    state.sel_start = sel.acpStart;
                    state.sel_end = sel.acpEnd;
                }
                Ok(())
            }
        )
    }

    fn GetText(
        &self,
        acpstart: i32,
        acpend: i32,
        pchplain: PWSTR,
        cchplainreq: u32,
        pcchplainret: *mut u32,
        prgruninfo: *mut TS_RUNINFO,
        cruninforeq: u32,
        pcruninforet: *mut u32,
        pacpnext: *mut i32,
    ) -> WinResult<()> {
        ffi_guard!(self.state.event_sink.clone(), "ITextStoreACP::GetText", {
            require_buffer(pchplain.0, cchplainreq)?;
            require_pointer(pcchplainret)?;
            require_buffer(prgruninfo, cruninforeq)?;
            require_pointer(pcruninforet)?;
            require_pointer(pacpnext)?;
            let state = self.state.read()?;
            require_read_lock(&state)?;
            let end = if acpend == -1 {
                state.end_acp()
            } else {
                acpend
            };
            let range = state.text_range(acpstart, end)?;
            let available = range.end - range.start;
            let copy_len = available.min(cchplainreq as usize);
            if copy_len > 0 && !pchplain.is_null() {
                // SAFETY: pchplain 已由 require_buffer 验证非空且容量 ≥ cchplainreq；copy_len ≤ cchplainreq；源切片为存活 UTF-16 文本。
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        state.text[range.start..].as_ptr(),
                        pchplain.0,
                        copy_len,
                    );
                }
            }
            // SAFETY: 三个输出指针均已由 require_pointer 验证非空，prgruninfo 由 require_buffer 验证容量。
            unsafe {
                *pcchplainret = copy_len as u32;
                *pacpnext = (range.start + copy_len) as i32;
                if cruninforeq > 0 && !prgruninfo.is_null() {
                    *prgruninfo = TS_RUNINFO {
                        uCount: copy_len as u32,
                        r#type: TS_RT_PLAIN,
                    };
                    *pcruninforet = 1;
                } else if !pcruninforet.is_null() {
                    *pcruninforet = 0;
                }
            }
            Ok(())
        })
    }

    fn SetText(
        &self,
        _dwflags: u32,
        acpstart: i32,
        acpend: i32,
        pchtext: &PCWSTR,
        cch: u32,
    ) -> WinResult<TS_TEXTCHANGE> {
        ffi_guard!(self.state.event_sink.clone(), "ITextStoreACP::SetText", {
            let mut state = self.state.write()?;
            require_write_lock(&state)?;
            let insert = if cch == 0 {
                &[][..]
            } else if pchtext.0.is_null() {
                return Err(WinError::from(E_INVALIDARG));
            } else {
                // SAFETY: pchtext 非空已由 else-if 条件验证，cch 为调用方声明的元素数，切片生命周期仅限本次调用。
                unsafe { std::slice::from_raw_parts(pchtext.0, cch as usize) }
            };
            state.replace_range(acpstart, acpend, insert)
        })
    }

    fn GetFormattedText(
        &self,
        _acpstart: i32,
        _acpend: i32,
    ) -> WinResult<windows::Win32::System::Com::IDataObject> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::GetFormattedText",
            { Err(WinError::from(windows::Win32::Foundation::E_NOTIMPL)) }
        )
    }

    fn GetEmbedded(
        &self,
        _acppos: i32,
        _rguidservice: *const GUID,
        _riid: *const GUID,
    ) -> WinResult<windows::core::IUnknown> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::GetEmbedded",
            { Err(WinError::from(windows::Win32::Foundation::E_NOTIMPL)) }
        )
    }

    fn QueryInsertEmbedded(
        &self,
        _pguidservice: *const GUID,
        _pformatetc: *const windows::Win32::System::Com::FORMATETC,
    ) -> WinResult<BOOL> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::QueryInsertEmbedded",
            { Ok(false.into()) }
        )
    }

    fn InsertEmbedded(
        &self,
        _dwflags: u32,
        _acpstart: i32,
        _acpend: i32,
        _pdataobject: Ref<'_, windows::Win32::System::Com::IDataObject>,
    ) -> WinResult<TS_TEXTCHANGE> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::InsertEmbedded",
            { Err(WinError::from(windows::Win32::Foundation::E_NOTIMPL)) }
        )
    }

    fn InsertTextAtSelection(
        &self,
        dwflags: u32,
        pchtext: &PCWSTR,
        cch: u32,
        pacpstart: *mut i32,
        pacpend: *mut i32,
        pchange: *mut TS_TEXTCHANGE,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::InsertTextAtSelection",
            {
                let returns_range =
                    dwflags & TS_IAS_QUERYONLY != 0 || dwflags & TS_IAS_NOQUERY == 0;
                if returns_range {
                    require_pointer(pacpstart)?;
                    require_pointer(pacpend)?;
                }
                let mut state = self.state.write()?;
                require_write_lock(&state)?;
                let start = state.sel_start;
                let end = state.sel_end;
                if dwflags & TS_IAS_QUERYONLY != 0 {
                    // SAFETY: pacpstart/pacpend 已由 returns_range 分支的 require_pointer 验证非空。
                    unsafe {
                        *pacpstart = start;
                        *pacpend = end;
                    }
                    return Ok(());
                }
                let insert = if cch == 0 {
                    &[][..]
                } else if pchtext.0.is_null() {
                    return Err(WinError::from(E_INVALIDARG));
                } else {
                    // SAFETY: pchtext 非空已由 else-if 条件验证，cch 为调用方声明的元素数。
                    unsafe { std::slice::from_raw_parts(pchtext.0, cch as usize) }
                };
                let change = state.replace_range(start, end, insert)?;
                if dwflags & TS_IAS_NOQUERY == 0 {
                    // SAFETY: pacpstart/pacpend 已由 returns_range 分支的 require_pointer 验证非空。
                    unsafe {
                        *pacpstart = change.acpStart;
                        *pacpend = change.acpNewEnd;
                    }
                }
                if !pchange.is_null() {
                    // SAFETY: pchange 非空已由条件验证，指向调用方分配的可写 TS_TEXTCHANGE。
                    unsafe {
                        *pchange = change;
                    }
                }
                Ok(())
            }
        )
    }

    fn InsertEmbeddedAtSelection(
        &self,
        _dwflags: u32,
        _pdataobject: Ref<'_, windows::Win32::System::Com::IDataObject>,
        _pacpstart: *mut i32,
        _pacpend: *mut i32,
        _pchange: *mut TS_TEXTCHANGE,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::InsertEmbeddedAtSelection",
            { Err(WinError::from(windows::Win32::Foundation::E_NOTIMPL)) }
        )
    }

    fn RequestSupportedAttrs(
        &self,
        _dwflags: u32,
        cfilterattrs: u32,
        pafilterattrs: *const GUID,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::RequestSupportedAttrs",
            {
                require_buffer(pafilterattrs, cfilterattrs)?;
                Ok(())
            }
        )
    }

    fn RequestAttrsAtPosition(
        &self,
        _acppos: i32,
        cfilterattrs: u32,
        pafilterattrs: *const GUID,
        _dwflags: u32,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::RequestAttrsAtPosition",
            {
                require_buffer(pafilterattrs, cfilterattrs)?;
                Ok(())
            }
        )
    }

    fn RequestAttrsTransitioningAtPosition(
        &self,
        _acppos: i32,
        cfilterattrs: u32,
        pafilterattrs: *const GUID,
        _dwflags: u32,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::RequestAttrsTransitioningAtPosition",
            {
                require_buffer(pafilterattrs, cfilterattrs)?;
                Ok(())
            }
        )
    }

    fn FindNextAttrTransition(
        &self,
        _acpstart: i32,
        _acphalt: i32,
        cfilterattrs: u32,
        pafilterattrs: *const GUID,
        _dwflags: u32,
        pacpnext: *mut i32,
        pffound: *mut BOOL,
        plfoundoffset: *mut i32,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::FindNextAttrTransition",
            {
                require_buffer(pafilterattrs, cfilterattrs)?;
                require_pointer(pacpnext)?;
                require_pointer(pffound)?;
                require_pointer(plfoundoffset)?;
                // SAFETY: 三个输出指针均已由 require_pointer 验证非空，指向调用方分配的可写存储。
                unsafe {
                    *pacpnext = 0;
                    *pffound = false.into();
                    *plfoundoffset = 0;
                }
                Ok(())
            }
        )
    }

    fn RetrieveRequestedAttrs(
        &self,
        ulcount: u32,
        paattrvals: *mut windows::Win32::UI::TextServices::TS_ATTRVAL,
        pcfetched: *mut u32,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::RetrieveRequestedAttrs",
            {
                require_buffer(paattrvals, ulcount)?;
                require_pointer(pcfetched)?;
                // SAFETY: pcfetched 已由 require_pointer 验证非空。
                unsafe {
                    *pcfetched = 0;
                }
                Ok(())
            }
        )
    }

    fn GetEndACP(&self) -> WinResult<i32> {
        ffi_guard!(self.state.event_sink.clone(), "ITextStoreACP::GetEndACP", {
            let state = self.state.read()?;
            require_read_lock(&state)?;
            Ok(state.end_acp())
        })
    }

    fn GetActiveView(&self) -> WinResult<u32> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::GetActiveView",
            { Ok(VIEW_ID) }
        )
    }

    fn GetACPFromPoint(
        &self,
        _vcview: u32,
        ptscreen: *const POINT,
        _dwflags: u32,
    ) -> WinResult<i32> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::GetACPFromPoint",
            {
                require_pointer(ptscreen)?;
                let state = self.state.read()?;
                require_read_lock(&state)?;
                Ok(state.end_acp())
            }
        )
    }

    fn GetTextExt(
        &self,
        _vcview: u32,
        acpstart: i32,
        acpend: i32,
        prc: *mut RECT,
        pfclipped: *mut BOOL,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::GetTextExt",
            {
                if prc.is_null() || pfclipped.is_null() {
                    return Err(WinError::from(E_INVALIDARG));
                }
                let state = self.state.read()?;
                require_read_lock(&state)?;
                state.text_range(acpstart, acpend)?;
                let rect = state.cursor_screen_rect()?;
                // SAFETY: prc/pfclipped 非空已由上方条件验证，指向调用方分配的可写结构。
                unsafe {
                    *prc = rect;
                    *pfclipped = false.into();
                }
                Ok(())
            }
        )
    }

    fn GetScreenExt(&self, _vcview: u32) -> WinResult<RECT> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITextStoreACP::GetScreenExt",
            {
                let state = self.state.read()?;
                state.screen_extent()
            }
        )
    }

    fn GetWnd(&self, _vcview: u32) -> WinResult<HWND> {
        ffi_guard!(self.state.event_sink.clone(), "ITextStoreACP::GetWnd", {
            let state = self.state.read()?;
            Ok(state.event_sink.hwnd)
        })
    }
}

impl ITfContextOwnerCompositionSink_Impl for TsfTextStore_Impl {
    fn OnStartComposition(&self, _pcomposition: Ref<'_, ITfCompositionView>) -> WinResult<BOOL> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITfContextOwnerCompositionSink::OnStartComposition",
            {
                let mut state = self.state.write()?;
                let events = tsf_composition_events(&mut state.composition, Some(""), None, false);
                // empty marked is ignored by on_marked_text — force start via empty update path:
                if !state.composition.active {
                    state.composition.active = true;
                    state
                        .event_sink
                        .push(vec![UiEvent::ime_composition_start()]);
                }
                let _ = events;
                Ok(true.into())
            }
        )
    }

    fn OnUpdateComposition(
        &self,
        _pcomposition: Ref<'_, ITfCompositionView>,
        _prangenew: Ref<'_, windows::Win32::UI::TextServices::ITfRange>,
    ) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITfContextOwnerCompositionSink::OnUpdateComposition",
            {
                let mut state = self.state.write()?;
                let marked = state.utf16_string();
                if marked.is_empty() {
                    return Ok(());
                }
                let events =
                    tsf_composition_events(&mut state.composition, Some(&marked), None, false);
                state.event_sink.push(events);
                Ok(())
            }
        )
    }

    fn OnEndComposition(&self, _pcomposition: Ref<'_, ITfCompositionView>) -> WinResult<()> {
        ffi_guard!(
            self.state.event_sink.clone(),
            "ITfContextOwnerCompositionSink::OnEndComposition",
            {
                let mut state = self.state.write()?;
                let committed = state.utf16_string();
                let events = if committed.is_empty() {
                    tsf_composition_events(&mut state.composition, None, None, true)
                } else {
                    tsf_composition_events(&mut state.composition, None, Some(&committed), false)
                };
                state.event_sink.push(events);
                state.text.clear();
                state.sel_start = 0;
                state.sel_end = 0;
                Ok(())
            }
        )
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../tests-src/native/backends/windows/tsf_text_store_tests.rs"]
mod tsf_text_store_tests;

// 宏依赖的 panic 注入钩子：测试版位于 tests-src（模块级 include! 保持宏体作用域），
// 生产空钩子留在源内。
#[cfg(test)]
include!("../../../../tests-src/native/backends/windows/tsf_panic_hook_test.rs");

#[cfg(not(test))]
#[inline]
fn panic_if_requested(_: &str) {}

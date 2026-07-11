//! Minimal `ITextStoreACP` + `ITfContextOwnerCompositionSink`（P6 TSF phonetic）。
//!
//! TIP 经 lock 写入 UTF-16 缓冲；composition sink 把 marked/committed 收成 `UiEvent`。

#![cfg(windows)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use windows::core::{
    implement, ComObject, Error as WinError, Interface, Ref, Result as WinResult, BOOL, GUID,
    HRESULT, PCWSTR, PWSTR,
};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::UI::TextServices::{
    ITextStoreACP, ITextStoreACPSink, ITextStoreACP_Impl, ITfCompositionView,
    ITfContextOwnerCompositionSink, ITfContextOwnerCompositionSink_Impl, TEXT_STORE_LOCK_FLAGS,
    TS_AE_END, TS_AS_SEL_CHANGE, TS_AS_TEXT_CHANGE, TS_E_NOLOCK, TS_E_SYNCHRONOUS, TS_IAS_NOQUERY,
    TS_IAS_QUERYONLY, TS_LF_READWRITE, TS_RT_PLAIN, TS_RUNINFO, TS_SELECTIONSTYLE,
    TS_SELECTION_ACP, TS_SS_NOHIDDENTEXT, TS_SS_TRANSITORY, TS_STATUS, TS_TEXTCHANGE,
};

use crate::core::WindowId;
use crate::native::backends::windows::tsf_session::tsf_composition_events;
use crate::native::shared::ime_events::ImeCompositionState;
use crate::native::traits::event::UiEvent;

const VIEW_ID: u32 = 0;

#[derive(Clone)]
pub(crate) struct TsfEventSink {
    pub events: Arc<Mutex<VecDeque<UiEvent>>>,
    pub window_id: WindowId,
    pub hwnd: HWND,
}

impl TsfEventSink {
    fn push(&self, events: Vec<UiEvent>) {
        if events.is_empty() {
            return;
        }
        if let Ok(mut q) = self.events.lock() {
            for mut event in events {
                event.window_id = Some(self.window_id);
                q.push_back(event);
            }
        }
        // 唤醒主循环（与 EventLoopWaker 同形）。
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(self.hwnd),
                windows::Win32::UI::WindowsAndMessaging::WM_NULL,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            );
        }
    }
}

pub(crate) struct TsfStoreState {
    text: Vec<u16>,
    sel_start: i32,
    sel_end: i32,
    locked: bool,
    acp_sink: Option<ITextStoreACPSink>,
    sink_mask: u32,
    composition: ImeCompositionState,
    cursor: RECT,
    event_sink: TsfEventSink,
}

impl TsfStoreState {
    pub(crate) fn new(event_sink: TsfEventSink) -> Self {
        Self {
            text: Vec::new(),
            sel_start: 0,
            sel_end: 0,
            locked: false,
            acp_sink: None,
            sink_mask: 0,
            composition: ImeCompositionState::default(),
            cursor: RECT {
                left: 0,
                top: 0,
                right: 1,
                bottom: 16,
            },
            event_sink,
        }
    }

    pub(crate) fn set_cursor_rect(&mut self, rect: RECT) {
        self.cursor = rect;
    }

    pub(crate) fn composition_active(&self) -> bool {
        self.composition.active
    }

    fn end_acp(&self) -> i32 {
        self.text.len() as i32
    }

    fn clamp_acp(&self, acp: i32) -> i32 {
        acp.clamp(0, self.end_acp())
    }

    fn selection(&self) -> TS_SELECTION_ACP {
        TS_SELECTION_ACP {
            acpStart: self.sel_start,
            acpEnd: self.sel_end,
            style: TS_SELECTIONSTYLE {
                ase: TS_AE_END,
                fInterimChar: false.into(),
            },
        }
    }

    fn replace_range(&mut self, start: i32, end: i32, insert: &[u16]) -> TS_TEXTCHANGE {
        let start = self.clamp_acp(start) as usize;
        let end = self.clamp_acp(end) as usize;
        let end = end.max(start);
        self.text.splice(start..end, insert.iter().copied());
        let new_end = (start + insert.len()) as i32;
        self.sel_start = new_end;
        self.sel_end = new_end;
        TS_TEXTCHANGE {
            acpStart: start as i32,
            acpOldEnd: end as i32,
            acpNewEnd: new_end,
        }
    }

    fn utf16_string(&self) -> String {
        String::from_utf16_lossy(&self.text)
    }
}

#[implement(ITextStoreACP, ITfContextOwnerCompositionSink)]
pub(crate) struct TsfTextStore {
    state: Arc<Mutex<TsfStoreState>>,
}

impl TsfTextStore {
    pub(crate) fn new(state: Arc<Mutex<TsfStoreState>>) -> Self {
        Self { state }
    }

    pub(crate) fn create(event_sink: TsfEventSink) -> (ComObject<Self>, Arc<Mutex<TsfStoreState>>) {
        let state = Arc::new(Mutex::new(TsfStoreState::new(event_sink)));
        let store = ComObject::new(Self::new(Arc::clone(&state)));
        (store, state)
    }
}

fn lock_err() -> WinError {
    WinError::from(TS_E_NOLOCK)
}

fn require_lock(state: &TsfStoreState) -> WinResult<()> {
    if state.locked {
        Ok(())
    } else {
        Err(lock_err())
    }
}

impl ITextStoreACP_Impl for TsfTextStore_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<'_, windows::core::IUnknown>, dwmask: u32) -> WinResult<()> {
        let iid = unsafe { *riid };
        if iid != ITextStoreACPSink::IID {
            return Ok(());
        }
        let punk = punk.ok()?;
        let sink: ITextStoreACPSink = punk.cast()?;
        let mut state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        state.acp_sink = Some(sink);
        state.sink_mask = dwmask;
        Ok(())
    }

    fn UnadviseSink(&self, punk: Ref<'_, windows::core::IUnknown>) -> WinResult<()> {
        let Ok(punk) = punk.ok() else {
            return Ok(());
        };
        let mut state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        if let Some(existing) = state.acp_sink.as_ref() {
            if Interface::as_raw(existing) == Interface::as_raw(punk) {
                state.acp_sink = None;
                state.sink_mask = 0;
            }
        }
        Ok(())
    }

    fn RequestLock(&self, dwlockflags: u32) -> WinResult<HRESULT> {
        let sink = {
            let mut state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
            if state.locked {
                return Ok(TS_E_SYNCHRONOUS);
            }
            state.locked = true;
            state.acp_sink.clone()
        };
        let session_hr = if let Some(sink) = sink {
            match unsafe { sink.OnLockGranted(TEXT_STORE_LOCK_FLAGS(dwlockflags)) } {
                Ok(()) => HRESULT(0),
                Err(err) => err.code(),
            }
        } else {
            HRESULT(0)
        };
        if let Ok(mut state) = self.state.lock() {
            state.locked = false;
        }
        let _ = dwlockflags & TS_LF_READWRITE.0;
        Ok(session_hr)
    }

    fn GetStatus(&self) -> WinResult<TS_STATUS> {
        Ok(TS_STATUS {
            dwDynamicFlags: 0,
            dwStaticFlags: TS_SS_TRANSITORY | TS_SS_NOHIDDENTEXT,
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
        unsafe {
            *pacpresultstart = acpteststart;
            *pacpresultend = acptestend;
        }
        Ok(())
    }

    fn GetSelection(
        &self,
        ulindex: u32,
        ulcount: u32,
        pselection: *mut TS_SELECTION_ACP,
        pcfetched: *mut u32,
    ) -> WinResult<()> {
        let state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        require_lock(&state)?;
        let mut fetched = 0u32;
        if ulcount > 0 && ulindex == 0 {
            unsafe {
                *pselection = state.selection();
            }
            fetched = 1;
        }
        unsafe {
            *pcfetched = fetched;
        }
        Ok(())
    }

    fn SetSelection(&self, ulcount: u32, pselection: *const TS_SELECTION_ACP) -> WinResult<()> {
        let mut state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        require_lock(&state)?;
        if ulcount > 0 {
            let sel = unsafe { *pselection };
            state.sel_start = state.clamp_acp(sel.acpStart);
            state.sel_end = state.clamp_acp(sel.acpEnd);
        }
        Ok(())
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
        let state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        require_lock(&state)?;
        let start = state.clamp_acp(acpstart) as usize;
        let end = if acpend == -1 {
            state.text.len()
        } else {
            state.clamp_acp(acpend) as usize
        };
        let end = end.max(start);
        let available = end - start;
        let copy_len = available.min(cchplainreq as usize);
        if copy_len > 0 && !pchplain.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(state.text[start..].as_ptr(), pchplain.0, copy_len);
            }
        }
        unsafe {
            *pcchplainret = copy_len as u32;
            *pacpnext = (start + copy_len) as i32;
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
    }

    fn SetText(
        &self,
        _dwflags: u32,
        acpstart: i32,
        acpend: i32,
        pchtext: &PCWSTR,
        cch: u32,
    ) -> WinResult<TS_TEXTCHANGE> {
        let mut state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        require_lock(&state)?;
        let insert = unsafe { std::slice::from_raw_parts(pchtext.0, cch as usize) };
        Ok(state.replace_range(acpstart, acpend, insert))
    }

    fn GetFormattedText(
        &self,
        _acpstart: i32,
        _acpend: i32,
    ) -> WinResult<windows::Win32::System::Com::IDataObject> {
        Err(WinError::from(windows::Win32::Foundation::E_NOTIMPL))
    }

    fn GetEmbedded(
        &self,
        _acppos: i32,
        _rguidservice: *const GUID,
        _riid: *const GUID,
    ) -> WinResult<windows::core::IUnknown> {
        Err(WinError::from(windows::Win32::Foundation::E_NOTIMPL))
    }

    fn QueryInsertEmbedded(
        &self,
        _pguidservice: *const GUID,
        _pformatetc: *const windows::Win32::System::Com::FORMATETC,
    ) -> WinResult<BOOL> {
        Ok(false.into())
    }

    fn InsertEmbedded(
        &self,
        _dwflags: u32,
        _acpstart: i32,
        _acpend: i32,
        _pdataobject: Ref<'_, windows::Win32::System::Com::IDataObject>,
    ) -> WinResult<TS_TEXTCHANGE> {
        Err(WinError::from(windows::Win32::Foundation::E_NOTIMPL))
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
        let mut state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        require_lock(&state)?;
        let start = state.sel_start;
        let end = state.sel_end;
        if dwflags & TS_IAS_QUERYONLY != 0 {
            unsafe {
                *pacpstart = start;
                *pacpend = end;
            }
            return Ok(());
        }
        let insert = if cch == 0 || pchtext.0.is_null() {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(pchtext.0, cch as usize) }
        };
        let change = state.replace_range(start, end, insert);
        if dwflags & TS_IAS_NOQUERY == 0 {
            unsafe {
                *pacpstart = change.acpStart;
                *pacpend = change.acpNewEnd;
            }
        }
        if !pchange.is_null() {
            unsafe {
                *pchange = change;
            }
        }
        Ok(())
    }

    fn InsertEmbeddedAtSelection(
        &self,
        _dwflags: u32,
        _pdataobject: Ref<'_, windows::Win32::System::Com::IDataObject>,
        _pacpstart: *mut i32,
        _pacpend: *mut i32,
        _pchange: *mut TS_TEXTCHANGE,
    ) -> WinResult<()> {
        Err(WinError::from(windows::Win32::Foundation::E_NOTIMPL))
    }

    fn RequestSupportedAttrs(
        &self,
        _dwflags: u32,
        _cfilterattrs: u32,
        _pafilterattrs: *const GUID,
    ) -> WinResult<()> {
        Ok(())
    }

    fn RequestAttrsAtPosition(
        &self,
        _acppos: i32,
        _cfilterattrs: u32,
        _pafilterattrs: *const GUID,
        _dwflags: u32,
    ) -> WinResult<()> {
        Ok(())
    }

    fn RequestAttrsTransitioningAtPosition(
        &self,
        _acppos: i32,
        _cfilterattrs: u32,
        _pafilterattrs: *const GUID,
        _dwflags: u32,
    ) -> WinResult<()> {
        Ok(())
    }

    fn FindNextAttrTransition(
        &self,
        _acpstart: i32,
        _acphalt: i32,
        _cfilterattrs: u32,
        _pafilterattrs: *const GUID,
        _dwflags: u32,
        pacpnext: *mut i32,
        pffound: *mut BOOL,
        plfoundoffset: *mut i32,
    ) -> WinResult<()> {
        unsafe {
            *pacpnext = 0;
            *pffound = false.into();
            *plfoundoffset = 0;
        }
        Ok(())
    }

    fn RetrieveRequestedAttrs(
        &self,
        _ulcount: u32,
        _paattrvals: *mut windows::Win32::UI::TextServices::TS_ATTRVAL,
        pcfetched: *mut u32,
    ) -> WinResult<()> {
        unsafe {
            *pcfetched = 0;
        }
        Ok(())
    }

    fn GetEndACP(&self) -> WinResult<i32> {
        let state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        Ok(state.end_acp())
    }

    fn GetActiveView(&self) -> WinResult<u32> {
        Ok(VIEW_ID)
    }

    fn GetACPFromPoint(
        &self,
        _vcview: u32,
        _ptscreen: *const POINT,
        _dwflags: u32,
    ) -> WinResult<i32> {
        let state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        Ok(state.end_acp())
    }

    fn GetTextExt(
        &self,
        _vcview: u32,
        _acpstart: i32,
        _acpend: i32,
        prc: *mut RECT,
        pfclipped: *mut BOOL,
    ) -> WinResult<()> {
        let state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        unsafe {
            *prc = state.cursor;
            *pfclipped = false.into();
        }
        Ok(())
    }

    fn GetScreenExt(&self, _vcview: u32) -> WinResult<RECT> {
        let state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        let mut rect = state.cursor;
        let mut tl = POINT {
            x: rect.left,
            y: rect.top,
        };
        let mut br = POINT {
            x: rect.right,
            y: rect.bottom,
        };
        unsafe {
            let _ = windows::Win32::Graphics::Gdi::ClientToScreen(state.event_sink.hwnd, &mut tl);
            let _ = windows::Win32::Graphics::Gdi::ClientToScreen(state.event_sink.hwnd, &mut br);
        }
        rect.left = tl.x;
        rect.top = tl.y;
        rect.right = br.x;
        rect.bottom = br.y;
        Ok(rect)
    }

    fn GetWnd(&self, _vcview: u32) -> WinResult<HWND> {
        let state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        Ok(state.event_sink.hwnd)
    }
}

impl ITfContextOwnerCompositionSink_Impl for TsfTextStore_Impl {
    fn OnStartComposition(
        &self,
        _pcomposition: Ref<'_, ITfCompositionView>,
    ) -> WinResult<BOOL> {
        let mut state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        let events = tsf_composition_events(&mut state.composition, Some(""), None, false);
        // empty marked is ignored by on_marked_text — force start via empty update path:
        if !state.composition.active {
            state.composition.active = true;
            state.event_sink.push(vec![UiEvent::ime_composition_start()]);
        }
        let _ = events;
        Ok(true.into())
    }

    fn OnUpdateComposition(
        &self,
        _pcomposition: Ref<'_, ITfCompositionView>,
        _prangenew: Ref<'_, windows::Win32::UI::TextServices::ITfRange>,
    ) -> WinResult<()> {
        let mut state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
        let marked = state.utf16_string();
        if marked.is_empty() {
            return Ok(());
        }
        let events = tsf_composition_events(&mut state.composition, Some(&marked), None, false);
        state.event_sink.push(events);
        Ok(())
    }

    fn OnEndComposition(&self, _pcomposition: Ref<'_, ITfCompositionView>) -> WinResult<()> {
        let mut state = self.state.lock().map_err(|_| WinError::from(HRESULT(0x8000_FFFF_u32 as i32)))?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::WindowId;

    fn test_state() -> TsfStoreState {
        TsfStoreState::new(TsfEventSink {
            events: Arc::new(Mutex::new(VecDeque::new())),
            window_id: WindowId::new(1),
            hwnd: HWND(std::ptr::null_mut()),
        })
    }

    #[test]
    fn replace_range_updates_selection_and_buffer() {
        let mut state = test_state();
        let change = state.replace_range(0, 0, &[b'z' as u16, b'h' as u16]);
        assert_eq!(state.utf16_string(), "zh");
        assert_eq!(change.acpNewEnd, 2);
        assert_eq!(state.sel_end, 2);
    }

    #[test]
    fn advise_mask_constants_cover_text_and_selection() {
        assert_ne!(TS_AS_TEXT_CHANGE, 0);
        assert_ne!(TS_AS_SEL_CHANGE, 0);
    }
}
